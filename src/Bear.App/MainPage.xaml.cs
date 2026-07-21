using Bear.App.Theming;
using Chess;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Pickers;

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
        AddAppMenuItems();
        RestorePalette();
        UpdateClockText();
        UpdateStatus();
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
        var copyPgn = new MenuFlyoutItem { Text = "Copy PGN" };
        copyPgn.Click += (_, _) =>
        {
            var package = new DataPackage { RequestedOperation = DataPackageOperation.Copy };
            package.SetText(BuildSessionPgn());
            Clipboard.SetContent(package);
            _exported = true;
        };
        menu.Items.Insert(1, copyPgn);

        var savePgn = new MenuFlyoutItem { Text = "Save PGN as…" };
        savePgn.Click += async (_, _) => await SavePgnAsync();
        menu.Items.Insert(2, savePgn);

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
                OpponentText.Text = $"Opponent {value}";
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
                item.Click += (_, _) =>
                {
                    _baseTime = TimeSpan.FromMinutes(minutes);
                    _increment = TimeSpan.FromSeconds(incrementSeconds);
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
    private static (string Speed, (string Label, int Minutes, int Increment)[] Controls)[] TimeControls() =>
    [
        ("Blitz", [("3 + 0", 3, 0), ("3 + 2", 3, 2), ("5 + 0", 5, 0), ("5 + 3", 5, 3)]),
        ("Rapid", [("10 + 0", 10, 0), ("10 + 5", 10, 5), ("15 + 10", 15, 10), ("20 + 0", 20, 0)]),
    ];

    // --- the session log -------------------------------------------------

    /// <summary>
    /// Finished games from this run, oldest first. In memory only, and discarded on exit.
    ///
    /// A PGN file has always been able to hold many games, so collecting a session's worth
    /// costs one list and needs no store — which is the point. Writing them to disk as they
    /// finished would accumulate indefinitely and become a game database with extra steps,
    /// and that is deliberately not in scope. Nobody plays ten thousand games in a sitting.
    /// </summary>
    private readonly List<string> _sessionGames = [];

    /// <summary>Whether anything would be lost by quitting now.</summary>
    public bool HasUnexportedGames => _sessionGames.Count > 0 || BoardView.Board.ExecutedMoves.Count > 0;

    private bool _exported;

    /// <summary>
    /// Archive the outgoing game. Raised before the board is cleared, so
    /// <c>BoardView.Board</c> still holds it.
    /// </summary>
    private void OnGameEnding(object? sender, EventArgs e)
    {
        if (BoardView.Board.ExecutedMoves.Count == 0)
            return;

        _sessionGames.Add(BuildPgn());
        _exported = false;
    }

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

        // TODO(M5): colour is randomised per game, so this pairing becomes dynamic.
        var opponent = $"Maia {_opponentStrength}";

        var tags = new (string Key, string Value)[]
        {
            ("Event", "Casual game"),
            ("Site", "Bear"),
            ("Date", DateTime.Now.ToString("yyyy.MM.dd")),
            ("Round", "-"),
            ("White", "Player"),
            ("Black", opponent),
            ("Result", ResultTag()),
            // Seconds-plus-increment, per the PGN spec. This is the tag that makes an
            // exported game reconstructible: it fixes the category, and with it which
            // Maia model played.
            ("TimeControl", $"{(int)_baseTime.TotalSeconds}+{(int)_increment.TotalSeconds}"),
            // Only Maia's side carries a rating. Bear does not track the player's, and
            // omitting it is more honest than inventing one.
            ("BlackElo", _opponentStrength.ToString()),
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
    /// Every game of the session, the one in progress last. Games are separated by a blank
    /// line, which is what the PGN spec asks for and what readers expect.
    /// </summary>
    private string BuildSessionPgn()
    {
        var games = new List<string>(_sessionGames);

        if (BoardView.Board.ExecutedMoves.Count > 0)
            games.Add(BuildPgn());

        return games.Count == 0
            ? BuildPgn()
            : string.Join(Environment.NewLine + Environment.NewLine, games);
    }

    /// <summary>
    /// Offer to export before quitting. Returns true if it is safe to close.
    ///
    /// The session log lives only in memory, so closing really does discard it — which is
    /// exactly why this asks. Saving nothing is a legitimate answer and stays one click
    /// away; the dialog defaults to saving because that is the choice you cannot undo.
    /// </summary>
    public async Task<bool> ConfirmExitAsync()
    {
        if (!HasUnexportedGames || _exported)
            return true;

        var played = _sessionGames.Count + (BoardView.Board.ExecutedMoves.Count > 0 ? 1 : 0);

        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = played == 1 ? "Save this game?" : $"Save these {played} games?",
            Content = "The session's games haven't been exported. Closing discards them.",
            PrimaryButtonText = "Save PGN…",
            SecondaryButtonText = "Discard",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
        };

        switch (await dialog.ShowAsync())
        {
            case ContentDialogResult.Primary:
                await SavePgnAsync();
                // If the file picker was itself cancelled, nothing was written — stay open
                // rather than quietly dropping the games anyway.
                return _exported;
            case ContentDialogResult.Secondary:
                return true;
            default:
                return false;
        }
    }

    private async Task SavePgnAsync()
    {
        var picker = new FileSavePicker
        {
            SuggestedStartLocation = PickerLocationId.DocumentsLibrary,
            SuggestedFileName = $"bear-{DateTime.Now:yyyyMMdd-HHmm}",
        };
        picker.FileTypeChoices.Add("Portable Game Notation", [".pgn"]);

        // Unpackaged WinUI: a picker has no window to parent itself to unless one is
        // supplied, and it throws rather than guessing.
        if (App.Window is { } window)
        {
            WinRT.Interop.InitializeWithWindow.Initialize(
                picker, WinRT.Interop.WindowNative.GetWindowHandle(window));
        }

        if (await picker.PickSaveFileAsync() is { } file)
        {
            await FileIO.WriteTextAsync(file, BuildSessionPgn());
            _exported = true;
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

    private void OnGameReset(object sender, EventArgs e) => ResetClocks();

    private void OnBoardChanged(object sender, EventArgs e)
    {
        var board = BoardView.Board;

        // Any new move makes a previous export stale, so the exit prompt comes back.
        _exported = false;

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
