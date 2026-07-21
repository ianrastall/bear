using Bear.App.Controls;
using Bear.App.Maia;
using Bear.App.Theming;
using Chess;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;

namespace Bear.App;

/// <summary>
/// Hosts the board and the two clocks. No chess logic here — it reads state off the
/// board view and formats it.
/// </summary>
public sealed partial class MainPage : Page
{
    private readonly DispatcherTimer _ticker = new() { Interval = TimeSpan.FromMilliseconds(100) };
    private readonly Settings _settings = Settings.Load();

    private TimeSpan _baseTime = TimeSpan.FromMinutes(10);
    private TimeSpan _increment = TimeSpan.Zero;
    private TimeSpan _whiteRemaining;
    private TimeSpan _blackRemaining;
    private DateTime _lastTick;
    private bool _clockRunning;
    private int _opponentStrength = 1500;

    public MainPage()
    {
        InitializeComponent();

        _whiteRemaining = _baseTime;
        _blackRemaining = _baseTime;

        _ticker.Tick += OnTick;
        BoardView.GameEnding += OnGameEnding;
        BoardView.OpponentToMove += OnOpponentToMove;
        AddAppMenuItems();
        RestorePalette();
        UpdateClockText();
        UpdateStatus();
        StartGame();
    }

    // --- settings --------------------------------------------------------

    /// <summary>
    /// Apply the saved palette, then keep saving it. An unknown name — a palette renamed
    /// or dropped since the file was written — leaves the default in place.
    /// </summary>
    private void RestorePalette()
    {
        if (_settings.BoardPaletteCustom is { } hex
            && uint.TryParse(hex, System.Globalization.NumberStyles.HexNumber, null, out var rgb))
        {
            BoardView.Theme = BoardTheme.FromCustom(rgb);
        }
        else if (BoardTheme.ByName(_settings.BoardPalette) is { } saved)
        {
            BoardView.Theme = saved;
        }

        BoardView.ThemeChanged += (_, _) =>
        {
            var theme = BoardView.Theme;
            if (theme.IsCustom)
            {
                _settings.BoardPaletteCustom =
                    $"{theme.Dark.R:X2}{theme.Dark.G:X2}{theme.Dark.B:X2}";
            }
            else
            {
                _settings.BoardPalette = theme.Name;
                _settings.BoardPaletteCustom = null;
            }
            _settings.Save();
        };
    }

    // --- menu ------------------------------------------------------------

    /// <summary>Append app-level entries to the board's right-click menu.</summary>
    private void AddAppMenuItems()
    {
        var menu = BoardView.Menu;

        // File commands sit with New game at the top, not adrift at the bottom.
        // Index 1 is immediately after it.
        // Copy stays, because the clipboard is a different medium — pasting one game into
        // a chat or an import box is a thing the log on disk cannot do for you.
        var copyPgn = new MenuFlyoutItem { Text = "Copy PGN" };
        copyPgn.Click += (_, _) =>
        {
            var package = new DataPackage { RequestedOperation = DataPackageOperation.Copy };
            package.SetText(BuildPgn());
            Clipboard.SetContent(package);
        };
        menu.Items.Insert(1, copyPgn);

        var openFolder = new MenuFlyoutItem { Text = "Open games folder" };
        openFolder.Click += (_, _) => OpenGamesFolder();
        menu.Items.Insert(2, openFolder);

        menu.Items.Insert(3, new MenuFlyoutSeparator());

        menu.Items.Add(new MenuFlyoutSeparator());

        var strength = new MenuFlyoutSubItem { Text = "Opponent strength" };
        for (var elo = 1100; elo <= 1900; elo += 100)
        {
            var value = elo;
            var item = new RadioMenuFlyoutItem
            {
                Text = elo.ToString(),
                GroupName = "OpponentStrength",
                IsChecked = elo == _opponentStrength,
            };
            item.Click += (_, _) =>
            {
                _opponentStrength = value;
                UpdateOpponentText();
            };
            strength.Items.Add(item);
        }
        menu.Items.Add(strength);

        var time = new MenuFlyoutSubItem { Text = "Time control" };
        foreach (var (speed, controls) in TimeControls())
        {
            var group = new MenuFlyoutSubItem { Text = speed };
            foreach (var (label, minutes, incrementSeconds) in controls)
            {
                var item = new RadioMenuFlyoutItem
                {
                    Text = label,
                    GroupName = "TimeControl",
                    IsChecked = minutes == 10 && incrementSeconds == 0,
                };
                var speedOfGroup = SpeedOf(speed);
                item.Click += (_, _) =>
                {
                    _baseTime = TimeSpan.FromMinutes(minutes);
                    _increment = TimeSpan.FromSeconds(incrementSeconds);
                    _speed = speedOfGroup;
                    ResetClocks();
                };
                group.Items.Add(item);
            }
            time.Items.Add(group);
        }
        menu.Items.Add(time);
    }

    /// <summary>
    /// Time controls, grouped by the Maia 2 model that will play them — the group name *is*
    /// the weights file, so there is no mapping table to drift.
    ///
    /// The list is read off the models rather than chosen by taste. Each was trained on
    /// exactly one Lichess speed category, which Lichess assigns by estimated duration
    /// (<c>base + 40 × increment</c>): Blitz 180-479s, Rapid 480-1499s. Anything outside
    /// both windows would be played by a net that never saw a game at that speed, so it is
    /// not offered — that rules out 1+0 (60s, Bullet) and 30+0 (1800s, Classical), both of
    /// which this menu used to list.
    /// </summary>
    private static MaiaSpeed SpeedOf(string label) =>
        label == "Blitz" ? MaiaSpeed.Blitz : MaiaSpeed.Rapid;

    private static (string Speed, (string Label, int Minutes, int Increment)[] Controls)[] TimeControls() =>
    [
        ("Blitz", [("3 + 0", 3, 0), ("3 + 2", 3, 2), ("5 + 0", 5, 0), ("5 + 3", 5, 3)]),
        ("Rapid", [("10 + 0", 10, 0), ("10 + 5", 10, 5), ("15 + 10", 15, 10), ("20 + 0", 20, 0)]),
    ];

    // --- the opponent ----------------------------------------------------

    private MaiaEngine? _maia;
    private MaiaSpeed _speed = MaiaSpeed.Rapid;
    private bool _thinking;

    /// <summary>
    /// Load the model for the current time control, reusing the one already open when the
    /// speed has not changed. Loading is a second or two for 89 MB, so it is not something
    /// to do per move.
    /// </summary>
    private void EnsureEngine()
    {
        if (_maia is not null && _loadedSpeed == _speed)
            return;

        _maia?.Dispose();
        _maia = MaiaEngine.TryLoad(_speed);
        _loadedSpeed = _speed;
    }

    private MaiaSpeed? _loadedSpeed;

    /// <summary>
    /// Start a game with the player on a random side, and flip the board so they are
    /// always looking up it. If no model is embedded there is no opponent, and the board
    /// stays open to both sides rather than freezing.
    /// </summary>
    private void StartGame()
    {
        EnsureEngine();

        if (_maia is null)
        {
            BoardView.HumanColor = null;
            return;
        }

        var playsWhite = Random.Shared.Next(2) == 0;
        BoardView.HumanColor = playsWhite ? PieceColor.White : PieceColor.Black;

        if (BoardView.Flipped == playsWhite)
            BoardView.Flip();

        UpdateOpponentText();

        // Black to play means Maia opens, and nothing has moved to trigger that yet.
        if (!playsWhite)
            _ = MakeOpponentMoveAsync();
    }

    private void OnOpponentToMove(object? sender, EventArgs e) => _ = MakeOpponentMoveAsync();

    /// <summary>
    /// Maia's move: one forward pass, off the UI thread, played back on it.
    ///
    /// There is no artificial delay. Maia does not search, so its answer after ten seconds
    /// is its answer after four milliseconds, and stalling would be theatre.
    /// </summary>
    private async Task MakeOpponentMoveAsync()
    {
        if (_maia is null || _thinking || BoardView.Board.IsEndGame)
            return;

        if (BoardView.HumanColor is null || BoardView.Board.Turn == BoardView.HumanColor)
            return;

        _thinking = true;
        try
        {
            var fen = BoardView.Board.ToFen();
            var legal = BoardView.Board.Moves().Select(ChessBoardView.Uci).ToArray();
            if (legal.Length == 0)
                return;

            var engine = _maia;
            var elo = _opponentStrength;
            var uci = await Task.Run(() => engine.Play(fen, legal, elo));

            if (!BoardView.PlayUci(uci))
            {
                // The board is the authority on legality. If a move ever fails to match,
                // play something legal rather than stalling the game forever.
                BoardView.PlayUci(legal[0]);
            }
        }
        catch
        {
            // A failed evaluation must not wedge the game.
        }
        finally
        {
            _thinking = false;
        }
    }

    private void UpdateOpponentText()
    {
        var side = BoardView.HumanColor is null
            ? string.Empty
            : BoardView.HumanColor == PieceColor.White ? " — you are White" : " — you are Black";
        OpponentText.Text = $"Opponent {_opponentStrength}{side}";
    }

    // --- the game log ----------------------------------------------------

    /// <summary>
    /// Whether the game currently on the board has already been written out, so that a
    /// game finishing and then being replaced is not recorded twice.
    /// </summary>
    private bool _currentGameLogged;

    /// <summary>
    /// Write the current game to the log, once. Safe to call from every trigger — the
    /// game ending, a new game replacing it, or the window closing — because only the
    /// first call for a given game does anything.
    /// </summary>
    private void LogCurrentGame()
    {
        if (_currentGameLogged || BoardView.Board.ExecutedMoves.Count == 0)
            return;

        _currentGameLogged = true;
        GameLog.Append(BuildPgn());
    }

    /// <summary>
    /// Raised before the board is cleared, so <c>BoardView.Board</c> still holds the game
    /// being discarded — this is the last moment it can be recorded.
    /// </summary>
    private void OnGameEnding(object? sender, EventArgs e) => LogCurrentGame();

    /// <summary>Called by the window on close, to catch a game abandoned mid-play.</summary>
    public void FlushGameLog() => LogCurrentGame();

    // --- PGN -------------------------------------------------------------

    /// <summary>
    /// The game with a full tag pair section.
    ///
    /// <see cref="ChessBoard.ToPgn"/> emits its own header block, so its output is
    /// stripped to movetext and re-tagged here — the tags that matter (the clock, the
    /// opponent's rating) are the ones only this page knows.
    /// </summary>
    private string BuildPgn()
    {
        var board = BoardView.Board;

        var movetext = string.Join(
            Environment.NewLine,
            board.ToPgn()
                .Split('\n')
                .Select(l => l.TrimEnd('\r'))
                .SkipWhile(l => l.TrimStart().StartsWith('[') || l.Length == 0))
            .Trim();

        var opponent = $"Maia {_opponentStrength}";
        var humanIsWhite = BoardView.HumanColor != PieceColor.Black;

        var tags = new (string Key, string Value)[]
        {
            ("Event", "Casual game"),
            ("Site", "Bear"),
            ("Date", DateTime.Now.ToString("yyyy.MM.dd")),
            ("Round", "-"),
            ("White", humanIsWhite ? "Player" : opponent),
            ("Black", humanIsWhite ? opponent : "Player"),
            ("Result", ResultTag()),
            // Seconds-plus-increment, per the PGN spec. This is the tag that makes an
            // exported game reconstructible: it fixes the category, and with it which
            // Maia model played.
            ("TimeControl", $"{(int)_baseTime.TotalSeconds}+{(int)_increment.TotalSeconds}"),
            // Only Maia's side carries a rating. Bear does not track the player's, and
            // omitting it is more honest than inventing one.
            (humanIsWhite ? "BlackElo" : "WhiteElo", _opponentStrength.ToString()),
        };

        var header = string.Join(
            Environment.NewLine,
            tags.Select(t => $"[{t.Key} \"{t.Value}\"]"));

        return header + Environment.NewLine + Environment.NewLine + movetext;
    }

    private string ResultTag()
    {
        if (_whiteRemaining <= TimeSpan.Zero)
            return "0-1";
        if (_blackRemaining <= TimeSpan.Zero)
            return "1-0";

        var board = BoardView.Board;
        if (!board.IsEndGame)
            return "*";

        return board.EndGame?.WonSide switch
        {
            PieceColor { } side when side == PieceColor.White => "1-0",
            PieceColor { } side when side == PieceColor.Black => "0-1",
            _ => "1/2-1/2",
        };
    }

    /// <summary>
    /// Reveal the games folder. This replaces "Save PGN as…": with every game already on
    /// disk there is nothing to export, and what the player actually needs is to be told
    /// where the files are.
    /// </summary>
    private static void OpenGamesFolder()
    {
        try
        {
            Directory.CreateDirectory(GameLog.Folder);
            System.Diagnostics.Process.Start(
                new System.Diagnostics.ProcessStartInfo(GameLog.Folder) { UseShellExecute = true });
        }
        catch
        {
            // Explorer refusing to open is not worth interrupting a game for.
        }
    }

    // --- clocks ----------------------------------------------------------

    private void ResetClocks()
    {
        _ticker.Stop();
        _clockRunning = false;
        _whiteRemaining = _baseTime;
        _blackRemaining = _baseTime;
        UpdateClockText();
    }

    private void OnTick(object? sender, object e)
    {
        var now = DateTime.UtcNow;
        var elapsed = now - _lastTick;
        _lastTick = now;

        if (BoardView.Board.Turn == PieceColor.White)
            _whiteRemaining -= elapsed;
        else
            _blackRemaining -= elapsed;

        if (_whiteRemaining <= TimeSpan.Zero || _blackRemaining <= TimeSpan.Zero)
        {
            _whiteRemaining = _whiteRemaining < TimeSpan.Zero ? TimeSpan.Zero : _whiteRemaining;
            _blackRemaining = _blackRemaining < TimeSpan.Zero ? TimeSpan.Zero : _blackRemaining;
            _ticker.Stop();
            _clockRunning = false;
        }

        UpdateClockText();
        UpdateStatus();
    }

    private void UpdateClockText()
    {
        WhiteClockText.Text = Format(_whiteRemaining);
        BlackClockText.Text = Format(_blackRemaining);

        // Dim the side that isn't thinking.
        var whiteToMove = BoardView.Board.Turn == PieceColor.White;
        WhiteClockBox.Opacity = !_clockRunning || whiteToMove ? 1.0 : 0.45;
        BlackClockBox.Opacity = !_clockRunning || !whiteToMove ? 1.0 : 0.45;
    }

    /// <summary>mm:ss normally; switch to tenths under ten seconds.</summary>
    private static string Format(TimeSpan t)
    {
        if (t < TimeSpan.Zero)
            t = TimeSpan.Zero;

        return t < TimeSpan.FromSeconds(10)
            ? $"{t.Seconds}.{t.Milliseconds / 100}"
            : $"{(int)t.TotalMinutes}:{t.Seconds:00}";
    }

    // --- board events ----------------------------------------------------

    private void OnGameReset(object sender, EventArgs e)
    {
        ResetClocks();
        StartGame();
    }

    private void OnBoardChanged(object sender, EventArgs e)
    {
        var board = BoardView.Board;

        // A fresh position is a game that has not been recorded yet.
        _currentGameLogged = false;

        if (board.ExecutedMoves.Count > 0 && !_clockRunning && !board.IsEndGame)
        {
            _clockRunning = true;
            _lastTick = DateTime.UtcNow;
            _ticker.Start();
        }
        else if (_clockRunning)
        {
            // The side that just moved gets its increment.
            if (board.Turn == PieceColor.White)
                _blackRemaining += _increment;
            else
                _whiteRemaining += _increment;
            _lastTick = DateTime.UtcNow;
        }

        if (board.IsEndGame)
        {
            _ticker.Stop();
            _clockRunning = false;
        }

        UpdateClockText();
        UpdateStatus();
    }

    private void UpdateStatus()
    {
        var board = BoardView.Board;

        if (_whiteRemaining <= TimeSpan.Zero)
            StatusText.Text = "Black wins on time";
        else if (_blackRemaining <= TimeSpan.Zero)
            StatusText.Text = "White wins on time";
        else if (board.IsEndGame)
            StatusText.Text = DescribeEndGame(board);
        else if (board.WhiteKingChecked || board.BlackKingChecked)
            StatusText.Text = "Check";
        else
            StatusText.Text = string.Empty;
    }

    private static string DescribeEndGame(ChessBoard board)
    {
        var info = board.EndGame;
        if (info is null)
            return "Game over";

        return info.EndgameType switch
        {
            EndgameType.Checkmate => $"Checkmate — {info.WonSide} wins",
            EndgameType.Stalemate => "Draw — stalemate",
            EndgameType.FiftyMoveRule => "Draw — fifty-move rule",
            EndgameType.Repetition => "Draw — threefold repetition",
            EndgameType.InsufficientMaterial => "Draw — insufficient material",
            _ => $"Game over — {info.EndgameType}",
        };
    }
}
