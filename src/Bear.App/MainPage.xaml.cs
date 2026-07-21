using Bear.App.Theming;
using Chess;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

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
