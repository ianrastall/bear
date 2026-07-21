using Microsoft.ML.OnnxRuntime;
using Microsoft.ML.OnnxRuntime.Tensors;

namespace Bear.App.Maia;

/// <summary>Which Maia 2 model plays: the two are trained on disjoint Lichess speeds.</summary>
internal enum MaiaSpeed
{
    Blitz,
    Rapid,
}

/// <summary>
/// Maia 2, evaluated in-process. One forward pass per move — there is no search, no
/// pondering and nothing to configure.
///
/// The models are embedded resources when present. They are **not** in the repository
/// (89 MB each, and `artifacts/` is ignored), so a clone builds and runs without them and
/// simply has no opponent: <see cref="TryLoad"/> returns null rather than throwing. Run
/// `tools/export_onnx.py` to produce them.
/// </summary>
internal sealed class MaiaEngine : IDisposable
{
    private readonly InferenceSession _session;
    private readonly MaiaPolicy _policy;

    private MaiaEngine(InferenceSession session, MaiaPolicy policy)
    {
        _session = session;
        _policy = policy;
    }

    public static string ResourceName(MaiaSpeed speed) =>
        $"Bear.App.Assets.maia2-{speed.ToString().ToLowerInvariant()}.onnx";

    /// <summary>
    /// Load from a file on disk. Used by the verification harness, which checks the whole
    /// pipeline against the Python without needing the app's embedded resources.
    /// </summary>
    public static MaiaEngine FromFile(string modelPath, MaiaPolicy policy) =>
        new(new InferenceSession(modelPath, Options()), policy);

    /// <summary>
    /// Single-threaded: one 4-6 ms forward pass per move gains nothing from a thread pool,
    /// and spinning one up per session is pure overhead.
    /// </summary>
    private static SessionOptions Options() => new()
    {
        IntraOpNumThreads = 1,
        InterOpNumThreads = 1,
        GraphOptimizationLevel = GraphOptimizationLevel.ORT_ENABLE_ALL,
    };

    /// <summary>
    /// Load a model, or return null if it was not embedded in this build. Bear stays
    /// playable either way — without a model it is simply two people at one board.
    /// </summary>
    public static MaiaEngine? TryLoad(MaiaSpeed speed)
    {
        try
        {
            var assembly = typeof(MaiaEngine).Assembly;
            using var stream = assembly.GetManifestResourceStream(ResourceName(speed));
            if (stream is null)
                return null;

            using var buffer = new MemoryStream((int)stream.Length);
            stream.CopyTo(buffer);

            return new MaiaEngine(new InferenceSession(buffer.ToArray(), Options()), MaiaPolicy.Load());
        }
        catch
        {
            // A missing or unreadable model must not stop the app from opening.
            return null;
        }
    }

    /// <summary>
    /// Maia's move for a position, in real (un-mirrored) UCI.
    ///
    /// <paramref name="legalUci"/> must be the position's legal moves in real coordinates;
    /// they are mirrored internally to match the frame the network sees. Masking is
    /// essential rather than tidy — the policy scores all 1880 entries and its unmasked
    /// best is frequently illegal.
    /// </summary>
    public string Play(string fen, IReadOnlyCollection<string> legalUci, int elo)
    {
        var input = MaiaEncoder.Encode(fen, out var mirrored);
        var bucket = MaiaEncoder.RatingBucket(elo);

        var boards = new DenseTensor<float>(input, [1, 18, 8, 8]);
        // Both ratings get the same bucket: Bear does not track the player's, and at
        // eleven 100-point buckets the difference would not survive binning anyway.
        var self = new DenseTensor<long>(new[] { bucket }, [1]);
        var oppo = new DenseTensor<long>(new[] { bucket }, [1]);

        using var results = _session.Run(
        [
            NamedOnnxValue.CreateFromTensor("boards", boards),
            NamedOnnxValue.CreateFromTensor("elo_self", self),
            NamedOnnxValue.CreateFromTensor("elo_oppo", oppo),
        ]);

        var policy = results.First(r => r.Name == "policy").AsTensor<float>();
        var logits = policy.ToArray();

        var mirroredLegal = mirrored
            ? legalUci.Select(MaiaEncoder.MirrorMove).ToArray()
            : legalUci.ToArray();

        var chosen = _policy.Select(logits, mirroredLegal);
        return mirrored ? MaiaEncoder.MirrorMove(chosen) : chosen;
    }

    public void Dispose() => _session.Dispose();
}
