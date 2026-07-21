using System.Text.Json;
using Bear.App.Maia;

// M4's acceptance test: the C# encoder must reproduce maia2's Python exactly.
//
// The board tensor is entirely 0/1, so "exactly" is meant literally — the comparison is on
// the set of one-valued cells, not a tolerance. Anything less would let a subtly wrong
// mirror or a transposed rank/file slip through and only show up as an opponent that plays
// strangely as Black.

var repo = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", ".."));
var goldenPath = Path.Combine(repo, "tools", "golden.json");
var movesPath = Path.Combine(repo, "src", "Bear.App", "Assets", "maia2-moves.txt");

if (!File.Exists(goldenPath) || !File.Exists(movesPath))
{
    Console.Error.WriteLine($"Missing inputs. Run: python tools/make_golden.py\n  {goldenPath}\n  {movesPath}");
    return 2;
}

var moves = File.ReadAllLines(movesPath).Where(l => l.Length > 0).ToArray();
var golden = JsonDocument.Parse(File.ReadAllText(goldenPath)).RootElement;

int checks = 0, failures = 0;

void Check(bool ok, string what)
{
    checks++;
    if (ok) return;
    failures++;
    Console.WriteLine($"  FAIL  {what}");
}

Check(moves.Length == golden.GetProperty("moves").GetInt32(),
    $"move list length {moves.Length} vs {golden.GetProperty("moves").GetInt32()}");

foreach (var c in golden.GetProperty("cases").EnumerateArray())
{
    var fen = c.GetProperty("fen").GetString()!;
    Console.WriteLine(fen);

    var tensor = MaiaEncoder.Encode(fen, out var mirrored);

    Check(mirrored == c.GetProperty("mirrored").GetBoolean(), "mirror flag");

    var expectedOnes = c.GetProperty("ones").EnumerateArray().Select(x => x.GetInt32()).ToArray();
    var actualOnes = Enumerable.Range(0, tensor.Length).Where(i => tensor[i] != 0f).ToArray();

    if (!expectedOnes.SequenceEqual(actualOnes))
    {
        var missing = expectedOnes.Except(actualOnes).Take(6).ToArray();
        var extra = actualOnes.Except(expectedOnes).Take(6).ToArray();
        Check(false,
            $"tensor: {actualOnes.Length} set vs {expectedOnes.Length} expected"
            + (missing.Length > 0 ? $"; missing [{string.Join(",", missing)}]" : "")
            + (extra.Length > 0 ? $"; extra [{string.Join(",", extra)}]" : ""));
    }
    else
    {
        checks++;
    }

    // Every value must be 0 or 1 — the Python tensor is a set of indicator planes.
    Check(tensor.All(v => v is 0f or 1f), "tensor is binary");

    foreach (var e in c.GetProperty("by_elo").EnumerateArray())
    {
        Check(MaiaEncoder.RatingBucket(e.GetProperty("elo_self").GetInt32())
              == e.GetProperty("bucket_self").GetInt64(), "rating bucket (self)");
        Check(MaiaEncoder.RatingBucket(e.GetProperty("elo_oppo").GetInt32())
              == e.GetProperty("bucket_oppo").GetInt64(), "rating bucket (oppo)");

        foreach (var t in e.GetProperty("top").EnumerateArray())
        {
            var idx = t.GetProperty("index").GetInt32();
            var uciMirrored = t.GetProperty("uci_mirrored").GetString()!;
            var uci = t.GetProperty("uci").GetString()!;

            Check(moves[idx] == uciMirrored, $"policy[{idx}] = {moves[idx]} vs {uciMirrored}");

            var unmirrored = mirrored ? MaiaEncoder.MirrorMove(uciMirrored) : uciMirrored;
            Check(unmirrored == uci, $"un-mirror {uciMirrored} -> {unmirrored}, expected {uci}");
        }
    }
}

// The decoder must pick the same move the model's own masked argmax picked.
var policy = new MaiaPolicy(moves);
Check(policy.Count == moves.Length, "policy count");

foreach (var c in golden.GetProperty("cases").EnumerateArray())
{
    var legalIdx = c.GetProperty("legal").EnumerateArray().Select(x => x.GetInt32()).ToArray();
    var legalUci = legalIdx.Select(i => moves[i]).ToArray();

    foreach (var e in c.GetProperty("by_elo").EnumerateArray())
    {
        // Rebuild a logit vector that reproduces the recorded ranking, then confirm
        // Select() lands on the same move the Python did.
        var top = e.GetProperty("top").EnumerateArray().ToArray();
        var logits = new float[moves.Length];
        Array.Fill(logits, -1e9f);
        for (var r = 0; r < top.Length; r++)
            logits[top[r].GetProperty("index").GetInt32()] = 100f - r;

        var picked = policy.Select(logits, legalUci);
        Check(picked == top[0].GetProperty("uci_mirrored").GetString(),
            $"select -> {picked}, expected {top[0].GetProperty("uci_mirrored").GetString()}");

        foreach (var t in top)
        {
            Check(policy.TryIndex(t.GetProperty("uci_mirrored").GetString()!, out var back)
                  && back == t.GetProperty("index").GetInt32(), "uci -> index round-trip");
        }
    }
}

// Mirroring must be its own inverse, or a move could survive the round trip subtly changed.
foreach (var m in moves)
{
    Check(MaiaEncoder.MirrorMove(MaiaEncoder.MirrorMove(m)) == m, $"mirror round-trip {m}");
}

Console.WriteLine();
Console.WriteLine($"{checks - failures}/{checks} checks passed");
if (failures > 0)
    Console.WriteLine($"{failures} FAILURES");
return failures == 0 ? 0 : 1;
