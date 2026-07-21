using System.Diagnostics;
using System.Text;
using System.Threading.Channels;

namespace Bear.Engine;

/// <summary>
/// Drives a UCI engine subprocess. UCI is a strict request/response protocol, so all
/// interactions are serialised through a single lock: one command is in flight at a time.
/// A background task drains stdout into a channel; each command reads from that channel
/// until it sees its terminating token (uciok / readyok / bestmove).
/// </summary>
public sealed class UciEngine : IChessEngine
{
    private readonly string _fileName;
    private readonly IReadOnlyList<string> _arguments;
    private readonly TimeSpan _handshakeTimeout;
    private readonly TimeSpan _quitTimeout;

    private readonly SemaphoreSlim _sync = new(1, 1);
    private Process? _process;
    private Channel<string>? _output;
    private Task? _readerTask;
    private volatile bool _processExited;

    public EngineIdentity Identity { get; private set; } = EngineIdentity.Unknown;
    public bool IsRunning { get; private set; }

    /// <param name="fileName">Path to the engine executable (e.g. lc0.exe), or a launcher such as "dotnet".</param>
    /// <param name="arguments">Process arguments (e.g. the path to a .dll, or engine flags).</param>
    public UciEngine(
        string fileName,
        IReadOnlyList<string>? arguments = null,
        TimeSpan? handshakeTimeout = null,
        TimeSpan? quitTimeout = null)
    {
        _fileName = fileName ?? throw new ArgumentNullException(nameof(fileName));
        _arguments = arguments ?? [];
        _handshakeTimeout = handshakeTimeout ?? TimeSpan.FromSeconds(15);
        _quitTimeout = quitTimeout ?? TimeSpan.FromSeconds(5);
    }

    public async Task StartAsync(CancellationToken cancellationToken = default)
    {
        if (IsRunning)
            throw new InvalidOperationException("Engine is already running.");

        var psi = new ProcessStartInfo
        {
            FileName = _fileName,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            CreateNoWindow = true,
            // Encoding.UTF8 emits a BOM preamble; on stdin that corrupts the first UCI
            // command (engines see U+FEFF followed by "uci" and never reply). Use BOM-less UTF-8.
            StandardOutputEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false),
            StandardInputEncoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false),
        };
        foreach (var arg in _arguments)
            psi.ArgumentList.Add(arg);

        var process = new Process { StartInfo = psi, EnableRaisingEvents = true };
        process.Exited += (_, _) =>
        {
            _processExited = true;
            _output?.Writer.TryComplete();
        };

        try
        {
            if (!process.Start())
                throw new EngineException($"Failed to start engine process '{_fileName}'.");
        }
        catch (Exception ex) when (ex is not EngineException)
        {
            process.Dispose();
            throw new EngineException($"Failed to start engine process '{_fileName}'.", ex);
        }

        _process = process;
        _output = Channel.CreateUnbounded<string>(new UnboundedChannelOptions
        {
            SingleReader = true,
            SingleWriter = true,
        });
        _readerTask = Task.Run(() => ReadLoopAsync(process, _output.Writer));

        using var timeout = new CancellationTokenSource(_handshakeTimeout);
        using var linked = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken, timeout.Token);

        try
        {
            await _sync.WaitAsync(linked.Token).ConfigureAwait(false);
            try
            {
                await SendLineAsync("uci", linked.Token).ConfigureAwait(false);
                Identity = await ReadHandshakeAsync(linked.Token).ConfigureAwait(false);
            }
            finally
            {
                _sync.Release();
            }
        }
        catch (OperationCanceledException) when (timeout.IsCancellationRequested)
        {
            await ForceStopAsync().ConfigureAwait(false);
            throw new EngineException($"Engine did not complete the UCI handshake within {_handshakeTimeout.TotalSeconds:0}s.");
        }
        catch
        {
            await ForceStopAsync().ConfigureAwait(false);
            throw;
        }

        IsRunning = true;
    }

    public async Task SetOptionAsync(string name, string value, CancellationToken cancellationToken = default)
    {
        EnsureRunning();
        await WithLockAsync(async ct =>
        {
            await SendLineAsync($"setoption name {name} value {value}", ct).ConfigureAwait(false);
            // setoption yields no reply; isready forces the engine to acknowledge it has applied it.
            await SendLineAsync("isready", ct).ConfigureAwait(false);
            await ReadUntilAsync(line => line == "readyok", ct).ConfigureAwait(false);
        }, cancellationToken).ConfigureAwait(false);
    }

    public async Task NewGameAsync(CancellationToken cancellationToken = default)
    {
        EnsureRunning();
        await WithLockAsync(async ct =>
        {
            await SendLineAsync("ucinewgame", ct).ConfigureAwait(false);
            await SendLineAsync("isready", ct).ConfigureAwait(false);
            await ReadUntilAsync(line => line == "readyok", ct).ConfigureAwait(false);
        }, cancellationToken).ConfigureAwait(false);
    }

    public async Task SetPositionAsync(string? startingFen, IReadOnlyList<string> movesUci, CancellationToken cancellationToken = default)
    {
        EnsureRunning();
        ArgumentNullException.ThrowIfNull(movesUci);

        var sb = new StringBuilder("position ");
        sb.Append(startingFen is null ? "startpos" : $"fen {startingFen}");
        if (movesUci.Count > 0)
        {
            sb.Append(" moves");
            foreach (var move in movesUci)
            {
                sb.Append(' ');
                sb.Append(move);
            }
        }

        await WithLockAsync(async ct =>
        {
            await SendLineAsync(sb.ToString(), ct).ConfigureAwait(false);
            // position yields no reply; sync so a subsequent Go can't race ahead of it.
            await SendLineAsync("isready", ct).ConfigureAwait(false);
            await ReadUntilAsync(line => line == "readyok", ct).ConfigureAwait(false);
        }, cancellationToken).ConfigureAwait(false);
    }

    public async Task<BestMove> GoAsync(SearchLimits limits, CancellationToken cancellationToken = default)
    {
        EnsureRunning();
        ArgumentNullException.ThrowIfNull(limits);

        return await WithLockAsync(async ct =>
        {
            await SendLineAsync(limits.ToGoArguments(), ct).ConfigureAwait(false);

            string? bestLine = null;
            try
            {
                bestLine = await ReadUntilAsync(line => line.StartsWith("bestmove", StringComparison.Ordinal), ct)
                    .ConfigureAwait(false);
            }
            catch (OperationCanceledException) when (!_processExited)
            {
                // Caller cancelled: ask the engine to stop, then consume the bestmove it must still emit.
                await SendLineAsync("stop", CancellationToken.None).ConfigureAwait(false);
                bestLine = await ReadUntilAsync(
                    line => line.StartsWith("bestmove", StringComparison.Ordinal),
                    CancellationToken.None).ConfigureAwait(false);
            }

            return ParseBestMove(bestLine);
        }, cancellationToken).ConfigureAwait(false);
    }

    // --- internals -------------------------------------------------------

    private static async Task ReadLoopAsync(Process process, ChannelWriter<string> writer)
    {
        try
        {
            var reader = process.StandardOutput;
            while (await reader.ReadLineAsync().ConfigureAwait(false) is { } line)
                writer.TryWrite(line);
        }
        catch (Exception ex)
        {
            writer.TryComplete(ex);
            return;
        }
        writer.TryComplete();
    }

    private async Task<EngineIdentity> ReadHandshakeAsync(CancellationToken ct)
    {
        string name = EngineIdentity.Unknown.Name;
        string author = EngineIdentity.Unknown.Author;

        await ReadUntilAsync(line =>
        {
            if (line.StartsWith("id name ", StringComparison.Ordinal))
                name = line["id name ".Length..].Trim();
            else if (line.StartsWith("id author ", StringComparison.Ordinal))
                author = line["id author ".Length..].Trim();
            return line == "uciok";
        }, ct).ConfigureAwait(false);

        return new EngineIdentity(name, author);
    }

    /// <summary>
    /// Read lines from the engine until <paramref name="isTerminator"/> matches, then return that line.
    /// The predicate is invoked for every line, so it can also harvest data (id lines, info lines).
    /// </summary>
    private async Task<string> ReadUntilAsync(Func<string, bool> isTerminator, CancellationToken ct)
    {
        var channel = _output ?? throw new EngineException("Engine output channel is not available.");
        try
        {
            while (await channel.Reader.WaitToReadAsync(ct).ConfigureAwait(false))
            {
                while (channel.Reader.TryRead(out var line))
                {
                    if (isTerminator(line))
                        return line;
                }
            }
        }
        catch (ChannelClosedException ex)
        {
            throw new EngineException("Engine closed its output stream unexpectedly (process crashed?).", ex);
        }

        // Channel completed without the terminator: the process died mid-command.
        throw new EngineException("Engine terminated before completing the command.");
    }

    private static BestMove ParseBestMove(string line)
    {
        // Formats: "bestmove e2e4", "bestmove e7e8q ponder d2d4", "bestmove (none)".
        var tokens = line.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        if (tokens.Length < 2)
            throw new EngineException($"Malformed bestmove line: '{line}'.");

        var move = tokens[1];
        if (move is "(none)" or "0000")
            return BestMove.None;

        string? ponder = null;
        for (var i = 2; i < tokens.Length - 1; i++)
        {
            if (tokens[i] == "ponder")
            {
                ponder = tokens[i + 1];
                break;
            }
        }
        return new BestMove(move, ponder);
    }

    private async Task SendLineAsync(string command, CancellationToken ct)
    {
        var process = _process;
        if (process is null || _processExited)
            throw new EngineException("Engine process is not available.");

        try
        {
            await process.StandardInput.WriteLineAsync(command.AsMemory(), ct).ConfigureAwait(false);
            await process.StandardInput.FlushAsync(ct).ConfigureAwait(false);
        }
        catch (Exception ex) when (ex is IOException or ObjectDisposedException)
        {
            throw new EngineException("Failed to write to the engine (process gone?).", ex);
        }
    }

    private async Task WithLockAsync(Func<CancellationToken, Task> action, CancellationToken ct)
    {
        await _sync.WaitAsync(ct).ConfigureAwait(false);
        try { await action(ct).ConfigureAwait(false); }
        finally { _sync.Release(); }
    }

    private async Task<T> WithLockAsync<T>(Func<CancellationToken, Task<T>> action, CancellationToken ct)
    {
        await _sync.WaitAsync(ct).ConfigureAwait(false);
        try { return await action(ct).ConfigureAwait(false); }
        finally { _sync.Release(); }
    }

    private void EnsureRunning()
    {
        if (!IsRunning)
            throw new InvalidOperationException("Engine is not running. Call StartAsync first.");
        if (_processExited)
            throw new EngineException("Engine process has exited.");
    }

    private async Task ForceStopAsync()
    {
        IsRunning = false;
        var process = _process;
        if (process is null)
            return;

        try
        {
            if (!process.HasExited)
            {
                process.Kill(entireProcessTree: true);
                await process.WaitForExitAsync().ConfigureAwait(false);
            }
        }
        catch { /* best-effort teardown */ }
        finally
        {
            _output?.Writer.TryComplete();
            if (_readerTask is not null)
            {
                try { await _readerTask.ConfigureAwait(false); } catch { /* ignore */ }
            }
            process.Dispose();
            _process = null;
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (_process is null)
            return;

        // Try a graceful quit first, then force.
        if (!_processExited && IsRunning)
        {
            try
            {
                await SendLineAsync("quit", CancellationToken.None).ConfigureAwait(false);
                using var cts = new CancellationTokenSource(_quitTimeout);
                await _process.WaitForExitAsync(cts.Token).ConfigureAwait(false);
            }
            catch { /* fall through to force */ }
        }

        await ForceStopAsync().ConfigureAwait(false);
        _sync.Dispose();
    }
}
