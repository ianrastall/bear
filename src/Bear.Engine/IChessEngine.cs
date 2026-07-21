namespace Bear.Engine;

/// <summary>
/// A UCI-speaking chess engine, abstracted so that lc0 (running Maia weights),
/// Stockfish, or any other UCI engine can be driven through the same surface.
/// The application only ever talks to this interface, never to a concrete engine.
/// </summary>
public interface IChessEngine : IAsyncDisposable
{
    /// <summary>Engine identity reported during the UCI handshake (populated after <see cref="StartAsync"/>).</summary>
    EngineIdentity Identity { get; }

    /// <summary>True once the process is running and the UCI handshake has completed.</summary>
    bool IsRunning { get; }

    /// <summary>Launch the engine process and complete the UCI handshake (<c>uci</c> -&gt; <c>uciok</c>).</summary>
    Task StartAsync(CancellationToken cancellationToken = default);

    /// <summary>Set a UCI option (e.g. <c>WeightsFile</c> for lc0). Must be called before the first search.</summary>
    Task SetOptionAsync(string name, string value, CancellationToken cancellationToken = default);

    /// <summary>Send <c>ucinewgame</c> and re-synchronise. Call at the start of each new game.</summary>
    Task NewGameAsync(CancellationToken cancellationToken = default);

    /// <summary>
    /// Set the current position. Pass <paramref name="startingFen"/> = <c>null</c> for the standard start
    /// position, or a FEN string otherwise. <paramref name="movesUci"/> are long-algebraic moves
    /// (e.g. "e2e4", "e7e8q") applied from that position.
    /// </summary>
    Task SetPositionAsync(string? startingFen, IReadOnlyList<string> movesUci, CancellationToken cancellationToken = default);

    /// <summary>
    /// Run a search under the given limits and return the engine's chosen move.
    /// For Maia this is always <see cref="SearchLimits.MaiaMove"/> (<c>go nodes 1</c>).
    /// </summary>
    Task<BestMove> GoAsync(SearchLimits limits, CancellationToken cancellationToken = default);
}

/// <summary>Engine name/author as reported by <c>id name</c> / <c>id author</c> during the handshake.</summary>
public sealed record EngineIdentity(string Name, string Author)
{
    public static readonly EngineIdentity Unknown = new("(unknown)", "(unknown)");
}

/// <summary>
/// Search constraints translated into the UCI <c>go</c> command. Only one bound is normally set.
/// </summary>
public sealed record SearchLimits
{
    public long? Nodes { get; init; }
    public int? Depth { get; init; }
    public int? MoveTimeMs { get; init; }

    /// <summary>The canonical Maia search: exactly one node, i.e. pure policy, no tree search.</summary>
    public static SearchLimits MaiaMove { get; } = new() { Nodes = 1 };

    public static SearchLimits ToDepth(int depth) => new() { Depth = depth };
    public static SearchLimits ForMilliseconds(int ms) => new() { MoveTimeMs = ms };

    /// <summary>Render the <c>go</c> argument list, e.g. "go nodes 1" or "go movetime 1000".</summary>
    internal string ToGoArguments()
    {
        var parts = new List<string>(4) { "go" };
        if (Nodes is { } n) { parts.Add("nodes"); parts.Add(n.ToString()); }
        if (Depth is { } d) { parts.Add("depth"); parts.Add(d.ToString()); }
        if (MoveTimeMs is { } t) { parts.Add("movetime"); parts.Add(t.ToString()); }
        return string.Join(' ', parts);
    }
}

/// <summary>
/// The result of a search. <see cref="Move"/> is <c>null</c> when the engine reports
/// <c>bestmove (none)</c> (no legal move: checkmate or stalemate).
/// </summary>
public sealed record BestMove(string? Move, string? Ponder)
{
    public bool HasMove => Move is not null;
    public static readonly BestMove None = new(null, null);
}

/// <summary>Thrown when the engine misbehaves: crashes, times out, or breaks the UCI protocol.</summary>
public sealed class EngineException : Exception
{
    public EngineException(string message) : base(message) { }
    public EngineException(string message, Exception inner) : base(message, inner) { }
}
