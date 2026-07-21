namespace Bear.App.Maia;

/// <summary>
/// The 1880-entry policy space: index to move, and logits to a chosen move.
///
/// The ordering is *generated, not derived*. It comes from python-chess's own move
/// generation for a lone queen and knight on each square, followed by the promotions —
/// an internal detail of that library that C# has no business trying to reproduce.
/// `tools/make_golden.py` writes the list out and it ships as an embedded resource.
///
/// Moves here are always in Maia's mirrored frame. Converting back to real coordinates is
/// <see cref="MaiaEncoder.MirrorMove"/>'s job, and only when the position was mirrored.
/// </summary>
internal sealed class MaiaPolicy
{
    private readonly string[] _moves;
    private readonly Dictionary<string, int> _index;

    public MaiaPolicy(string[] moves)
    {
        _moves = moves;
        _index = new Dictionary<string, int>(moves.Length, StringComparer.Ordinal);
        for (var i = 0; i < moves.Length; i++)
            _index[moves[i]] = i;
    }

    public int Count => _moves.Length;

    public string Uci(int index) => _moves[index];

    public bool TryIndex(string uci, out int index) => _index.TryGetValue(uci, out index);

    /// <summary>
    /// Load the list embedded in the app. Kept separate from the constructor so the
    /// verification harness can build one from a file without dragging in resources.
    /// </summary>
    public static MaiaPolicy Load()
    {
        var assembly = typeof(MaiaPolicy).Assembly;
        using var stream = assembly.GetManifestResourceStream("Bear.App.Assets.maia2-moves.txt")
            ?? throw new InvalidOperationException("maia2-moves.txt is not embedded.");
        using var reader = new StreamReader(stream);

        var moves = new List<string>(1880);
        while (reader.ReadLine() is { } line)
        {
            if (line.Length > 0)
                moves.Add(line);
        }
        return new MaiaPolicy([.. moves]);
    }

    /// <summary>
    /// The move Maia plays: the highest-scoring legal one.
    ///
    /// Masking to legal moves is not a nicety. The policy head scores all 1880 entries and
    /// its top pick is frequently illegal in a given position, so an unmasked argmax
    /// produces an illegal move rather than a bad one.
    /// </summary>
    /// <param name="logits">The policy output, <see cref="Count"/> long.</param>
    /// <param name="legalMirrored">Legal moves in UCI, already in the mirrored frame.</param>
    public string Select(ReadOnlySpan<float> logits, IReadOnlyCollection<string> legalMirrored)
    {
        if (legalMirrored.Count == 0)
            throw new ArgumentException("No legal moves.", nameof(legalMirrored));

        var best = string.Empty;
        var bestScore = float.NegativeInfinity;

        foreach (var uci in legalMirrored)
        {
            // A legal move missing from the policy space would be a bug in the move list,
            // not a position Bear should crash on: skip it and let the rest compete.
            if (!_index.TryGetValue(uci, out var i) || i >= logits.Length)
                continue;

            if (logits[i] > bestScore)
            {
                bestScore = logits[i];
                best = uci;
            }
        }

        // Every legal move fell outside the policy space. Play something legal rather than
        // nothing at all.
        return best.Length > 0 ? best : legalMirrored.First();
    }
}
