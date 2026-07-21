using Bear.App.Theming;
using Chess;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.ApplicationModel.DataTransfer;
using Windows.Foundation;

namespace Bear.App.Controls;

/// <summary>
/// The board, and deliberately nothing else. Pieces can be dragged or click-moved;
/// right-clicking anywhere opens the menu, so no surrounding chrome is needed.
/// Legality is delegated entirely to Gera.Chess — this control draws and dispatches.
/// </summary>
public sealed partial class ChessBoardView : UserControl
{
    private const int Files = 8;

    /// <summary>Border thickness as a fraction of one square (34/84 in the reference board).</summary>
    private const double BorderRatio = 0.405;

    /// <summary>Coordinate font size as a fraction of one square (20/84 in the reference board).</summary>
    private const double CoordFontRatio = 20.0 / 84.0;

    /// <summary>Pointer travel before a press turns into a drag rather than a click.</summary>
    private const double DragThresholdPx = 4.0;

    private readonly Grid[,] _cells = new Grid[Files, Files];
    private readonly Border[,] _tints = new Border[Files, Files];
    private readonly Ellipse[,] _markers = new Ellipse[Files, Files];
    private readonly Image?[,] _pieceImages = new Image?[Files, Files];

    private readonly CoordLabel[] _filesTop = new CoordLabel[Files];
    private readonly CoordLabel[] _filesBottom = new CoordLabel[Files];
    private readonly CoordLabel[] _ranksLeft = new CoordLabel[Files];
    private readonly CoordLabel[] _ranksRight = new CoordLabel[Files];

    private double _square;
    private double _border;
    private BoardTheme _theme = BoardTheme.Default;
    private bool _piecesReady;

    private Position? _selected;
    private Move[] _legalFromSelected = [];

    // Drag state.
    private Position? _pressSquare;
    private Point _pressPoint;
    private bool _dragging;
    private Image? _dragGhost;

    public ChessBoard Board { get; private set; } = new();

    /// <summary>True when the board is drawn from Black's side.</summary>
    public bool Flipped { get; private set; }

    public BoardTheme Theme
    {
        get => _theme;
        set
        {
            _theme = value;
            ApplyTheme();
            Render();
            ThemeChanged?.Invoke(this, EventArgs.Empty);
        }
    }

    /// <summary>
    /// The right-click menu. Exposed so the hosting page can append app-level entries
    /// (opponent strength, time control) to the same menu the board owns.
    /// </summary>
    public MenuFlyout Menu { get; } = new();

    /// <summary>Raised after any change to the position.</summary>
    public event EventHandler? BoardChanged;

    /// <summary>Raised when a brand-new game starts, so hosts can reset clocks.</summary>
    public event EventHandler? GameReset;

    /// <summary>
    /// Raised just *before* the board is replaced, while <see cref="Board"/> still holds
    /// the game being discarded. The session log listens here; by the time
    /// <see cref="GameReset"/> fires the moves are gone.
    /// </summary>
    public event EventHandler? GameEnding;

    /// <summary>Raised when the palette changes, so the host can persist the choice.</summary>
    public event EventHandler? ThemeChanged;

    public ChessBoardView()
    {
        InitializeComponent();
        BuildSquares();
        BuildCoordinates();
        BuildMenu();
        ApplyTheme();
        _ = LoadPiecesAsync();
    }

    // --- public commands -------------------------------------------------

    public void Reset()
    {
        // Before anything is cleared: the host archives the outgoing game here, and it
        // still needs Board to hold it.
        GameEnding?.Invoke(this, EventArgs.Empty);

        Board = new ChessBoard();
        ClearSelection();
        Render();
        BoardChanged?.Invoke(this, EventArgs.Empty);
        GameReset?.Invoke(this, EventArgs.Empty);
    }

    public void UndoLastMove()
    {
        if (Board.MoveIndex < 0)
            return;

        Board.Cancel();
        ClearSelection();
        Render();
        BoardChanged?.Invoke(this, EventArgs.Empty);
    }

    public void Flip()
    {
        Flipped = !Flipped;
        UpdateCoordinateText();
        Render();
    }

    // --- setup -----------------------------------------------------------

    private async Task LoadPiecesAsync()
    {
        await PieceImages.PreloadAsync();
        _piecesReady = true;
        Render();
    }

    private void BuildSquares()
    {
        for (var i = 0; i < Files; i++)
        {
            SquaresGrid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
            SquaresGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        }

        for (var row = 0; row < Files; row++)
        {
            for (var col = 0; col < Files; col++)
            {
                var tint = new Border();
                var marker = new Ellipse
                {
                    HorizontalAlignment = HorizontalAlignment.Center,
                    VerticalAlignment = VerticalAlignment.Center,
                    Visibility = Visibility.Collapsed,
                };

                var cell = new Grid();
                cell.Children.Add(tint);
                cell.Children.Add(marker);

                Grid.SetRow(cell, row);
                Grid.SetColumn(cell, col);
                SquaresGrid.Children.Add(cell);

                _cells[row, col] = cell;
                _tints[row, col] = tint;
                _markers[row, col] = marker;
            }
        }

        Layers.PointerPressed += OnPointerPressed;
        Layers.PointerMoved += OnPointerMoved;
        Layers.PointerReleased += OnPointerReleased;
        Layers.PointerCaptureLost += (_, _) => CancelDrag();
        Layers.RightTapped += OnRightTapped;
    }

    /// <summary>
    /// Algebraic coordinates on all four sides, each centred in the border thickness and
    /// aligned to its square's centre — the layout the reference board exports.
    /// </summary>
    private void BuildCoordinates()
    {
        for (var i = 0; i < Files; i++)
        {
            _filesTop[i] = AddCoordLabel();
            _filesBottom[i] = AddCoordLabel();
            _ranksLeft[i] = AddCoordLabel();
            _ranksRight[i] = AddCoordLabel();
        }
        UpdateCoordinateText();
    }

    private CoordLabel AddCoordLabel()
    {
        var text = new TextBlock
        {
            FontWeight = FontWeights.Bold,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center,
        };
        var box = new Grid();
        box.Children.Add(text);
        CoordCanvas.Children.Add(box);
        return new CoordLabel(box, text);
    }

    private void UpdateCoordinateText()
    {
        for (var i = 0; i < Files; i++)
        {
            // i indexes display position, so flipping reverses both sequences.
            var file = ((char)(Flipped ? 'h' - i : 'a' + i)).ToString();
            var rank = (Flipped ? i + 1 : Files - i).ToString();

            _filesTop[i].Text.Text = file;
            _filesBottom[i].Text.Text = file;
            _ranksLeft[i].Text.Text = rank;
            _ranksRight[i].Text.Text = rank;
        }
    }

    private void ApplyTheme()
    {
        var light = new SolidColorBrush(_theme.Light);
        var dark = new SolidColorBrush(_theme.Dark);
        var marker = new SolidColorBrush(_theme.Marker);

        for (var row = 0; row < Files; row++)
        {
            for (var col = 0; col < Files; col++)
            {
                _cells[row, col].Background = (row + col) % 2 == 0 ? light : dark;
                _markers[row, col].Fill = marker;
            }
        }

        BoardFrame.Background = new SolidColorBrush(_theme.Border);

        var coordBrush = new SolidColorBrush(BoardTheme.Coordinate);
        foreach (var label in AllCoordLabels())
            label.Text.Foreground = coordBrush;
    }

    private IEnumerable<CoordLabel> AllCoordLabels()
        => _filesTop.Concat(_filesBottom).Concat(_ranksLeft).Concat(_ranksRight);

    // --- layout ----------------------------------------------------------

    private void OnRootSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var available = Math.Min(Root.ActualWidth, Root.ActualHeight);
        if (available < Files)
            return;

        // Solve for a whole-pixel square size including both borders, so square edges
        // stay crisp: available >= 8*square + 2*(square*BorderRatio).
        _square = Math.Floor(available / (Files + (2 * BorderRatio)));
        if (_square < 1)
            return;

        _border = Math.Round(_square * BorderRatio);

        var inner = _square * Files;
        var total = inner + (2 * _border);

        Layers.Width = inner;
        Layers.Height = inner;
        BoardFrame.Width = total;
        BoardFrame.Height = total;
        CoordCanvas.Width = total;
        CoordCanvas.Height = total;

        var fontSize = Math.Max(8.0, _square * CoordFontRatio);

        for (var i = 0; i < Files; i++)
        {
            LayoutLabel(_filesTop[i], _border + (i * _square), 0, _square, _border, fontSize);
            LayoutLabel(_filesBottom[i], _border + (i * _square), total - _border, _square, _border, fontSize);
            LayoutLabel(_ranksLeft[i], 0, _border + (i * _square), _border, _square, fontSize);
            LayoutLabel(_ranksRight[i], total - _border, _border + (i * _square), _border, _square, fontSize);
        }

        for (var row = 0; row < Files; row++)
        {
            for (var col = 0; col < Files; col++)
            {
                var m = _markers[row, col];
                m.Width = m.Height = _square * 0.28;
            }
        }

        Render();
    }

    private static void LayoutLabel(CoordLabel label, double left, double top, double width, double height, double fontSize)
    {
        label.Box.Width = width;
        label.Box.Height = height;
        label.Text.FontSize = fontSize;
        Canvas.SetLeft(label.Box, left);
        Canvas.SetTop(label.Box, top);
    }

    // --- pointer ---------------------------------------------------------

    private void OnPointerPressed(object sender, PointerRoutedEventArgs e)
    {
        var point = e.GetCurrentPoint(Layers);
        if (point.Properties.IsRightButtonPressed)
            return;

        if (Board.IsEndGame || !TryHitSquare(point.Position, out var square))
            return;

        // If a piece is already selected and this square completes a legal move, play it.
        if (_selected is not null && MovesTo(square) is { Length: > 0 } candidates)
        {
            ResolveAndPlay(candidates, square);
            return;
        }

        var legal = Board.Moves(square);
        if (legal.Length > 0)
        {
            _selected = square;
            _legalFromSelected = legal;
            _pressSquare = square;
            _pressPoint = point.Position;
            Layers.CapturePointer(e.Pointer);
        }
        else
        {
            ClearSelection();
        }

        Render();
    }

    private void OnPointerMoved(object sender, PointerRoutedEventArgs e)
    {
        if (_pressSquare is null)
            return;

        var pos = e.GetCurrentPoint(Layers).Position;

        if (!_dragging)
        {
            var travelled = Math.Abs(pos.X - _pressPoint.X) + Math.Abs(pos.Y - _pressPoint.Y);
            if (travelled < DragThresholdPx)
                return;
            BeginDrag(_pressSquare.Value);
        }

        MoveGhost(pos);
    }

    private void OnPointerReleased(object sender, PointerRoutedEventArgs e)
    {
        if (!_dragging)
        {
            // A plain click: the selection made in PointerPressed stands.
            _pressSquare = null;
            Layers.ReleasePointerCapture(e.Pointer);
            return;
        }

        // Resolve the drop BEFORE releasing capture. ReleasePointerCapture raises
        // PointerCaptureLost synchronously, and that handler cancels the drag — so
        // releasing first tears the drag down before the move can be played, and no
        // dragged move ever completes. EndDrag first makes that handler a no-op.
        var pos = e.GetCurrentPoint(Layers).Position;
        var hit = TryHitSquare(pos, out var target);
        var candidates = hit ? MovesTo(target) : [];

        EndDrag();
        Layers.ReleasePointerCapture(e.Pointer);

        if (candidates.Length > 0)
            ResolveAndPlay(candidates, target);
        else
            Render(); // Dropped somewhere illegal: the piece snaps back.
    }

    private void BeginDrag(Position from)
    {
        var piece = Board[from];
        if (piece is null)
            return;

        var (row, col) = ToDisplay(from);
        if (_pieceImages[row, col] is { } resting)
            resting.Opacity = 0.0;

        _dragGhost = new Image
        {
            Source = PieceImages.Get(piece),
            Width = _square,
            Height = _square,
            IsHitTestVisible = false,
        };
        DragCanvas.Children.Add(_dragGhost);
        _dragging = true;
    }

    private void MoveGhost(Point pointerPos)
    {
        if (_dragGhost is null)
            return;

        Canvas.SetLeft(_dragGhost, pointerPos.X - (_square / 2));
        Canvas.SetTop(_dragGhost, pointerPos.Y - (_square / 2));
    }

    private void EndDrag()
    {
        if (_dragGhost is not null)
        {
            DragCanvas.Children.Remove(_dragGhost);
            _dragGhost = null;
        }
        _dragging = false;
        _pressSquare = null;
    }

    private void CancelDrag()
    {
        if (!_dragging)
        {
            _pressSquare = null;
            return;
        }
        EndDrag();
        Render();
    }

    private void PlayMove(Move move)
    {
        Board.Move(move);
        EndDrag();
        ClearSelection();
        Render();
        BoardChanged?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>
    /// The legal moves landing on <paramref name="target"/>. Normally one; a pawn reaching
    /// the last rank produces four moves to the same square, which is the only reason the
    /// promotion picker exists.
    /// </summary>
    private Move[] MovesTo(Position target) => _legalFromSelected
        .Where(m => m.NewPosition.X == target.X && m.NewPosition.Y == target.Y)
        .ToArray();

    /// <summary>Play the only move, or ask which piece first.</summary>
    private void ResolveAndPlay(Move[] candidates, Position target)
    {
        switch (candidates.Length)
        {
            case 0:
                return;
            case 1:
                PlayMove(candidates[0]);
                return;
            default:
                ShowPromotionPicker(candidates, target);
                return;
        }
    }

    /// <summary>
    /// Four pieces in a row, over the promotion square, drawn in the promoting side's
    /// colour. Dismissing without choosing abandons the move and the pawn snaps back —
    /// auto-queening a cancelled promotion would be the one irreversible surprise on the
    /// board.
    /// </summary>
    private void ShowPromotionPicker(Move[] candidates, Position target)
    {
        // San carries the promotion as "=Q"; order the offer by usefulness, not by
        // whatever order the move generator happened to produce.
        var order = new[] { 'Q', 'R', 'B', 'N' };
        var offered = order
            .Select(c => candidates.FirstOrDefault(
                m => m.San?.Contains($"={c}", StringComparison.Ordinal) == true))
            .Where(m => m is not null)
            .Select(m => m!)
            .ToArray();

        if (offered.Length == 0)
        {
            Render();
            return;
        }

        // The drag ghost has been torn down but the board underneath still shows the pawn
        // lifted off its square. Repaint before the flyout opens over it.
        Render();

        var mover = Board.Turn;
        var picked = false;
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 2 };

        var flyout = new Flyout
        {
            Content = row,
            Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.Auto,
        };

        foreach (var move in offered)
        {
            var kind = order.First(c => move.San!.Contains($"={c}", StringComparison.Ordinal));
            var button = new Button
            {
                Width = _square,
                Height = _square,
                Padding = new Thickness(2),
                Background = new SolidColorBrush(_theme.Light),
                BorderBrush = new SolidColorBrush(_theme.Border),
                Content = new Image
                {
                    Source = PieceImages.Get(new Piece(mover, PieceTypeFor(kind))),
                    Stretch = Stretch.Uniform,
                },
            };

            var chosen = move;
            button.Click += (_, _) =>
            {
                picked = true;
                flyout.Hide();
                PlayMove(chosen);
            };
            row.Children.Add(button);
        }

        // Dismissed without choosing: abandon the move entirely rather than assuming a
        // queen. Clearing the selection leaves the pawn where it started.
        flyout.Closed += (_, _) =>
        {
            if (picked)
                return;
            ClearSelection();
            Render();
        };

        var (displayRow, displayCol) = ToDisplay(target);
        flyout.ShowAt(_cells[displayRow, displayCol]);
    }

    private static PieceType PieceTypeFor(char san) => san switch
    {
        'Q' => PieceType.Queen,
        'R' => PieceType.Rook,
        'B' => PieceType.Bishop,
        _ => PieceType.Knight,
    };

    private void ClearSelection()
    {
        _selected = null;
        _legalFromSelected = [];
        _pressSquare = null;
    }

    private bool TryHitSquare(Point p, out Position square)
    {
        square = default;
        if (_square <= 0)
            return false;

        var col = (int)Math.Floor(p.X / _square);
        var row = (int)Math.Floor(p.Y / _square);
        if (row < 0 || row >= Files || col < 0 || col >= Files)
            return false;

        square = ToPosition(row, col);
        return true;
    }

    // --- rendering -------------------------------------------------------

    private void Render()
    {
        if (_square <= 0)
            return;

        var lastMove = Board.ExecutedMoves.Count > 0 ? Board.ExecutedMoves[^1] : null;

        Position? checkedKing = null;
        if (Board.WhiteKingChecked)
            checkedKing = Board.WhiteKing;
        else if (Board.BlackKingChecked)
            checkedKing = Board.BlackKing;

        PieceCanvas.Children.Clear();
        Array.Clear(_pieceImages);

        for (var row = 0; row < Files; row++)
        {
            for (var col = 0; col < Files; col++)
            {
                var pos = ToPosition(row, col);
                var piece = Board[pos];

                var isTarget = _legalFromSelected.Any(m => m.NewPosition.X == pos.X && m.NewPosition.Y == pos.Y);
                var isCapture = isTarget && piece is not null;

                var marker = _markers[row, col];
                marker.Visibility = isTarget ? Visibility.Visible : Visibility.Collapsed;
                // Quiet moves get a small centre dot; captures get a large ring-like disc,
                // so the two read differently without needing colour to carry the meaning.
                marker.Width = marker.Height = isCapture ? _square * 0.86 : _square * 0.28;
                marker.Opacity = isCapture ? 0.35 : 1.0;

                _tints[row, col].Background = TintFor(pos, checkedKing, lastMove);

                if (piece is not null && _piecesReady)
                    AddPieceImage(row, col, piece);
            }
        }
    }

    private SolidColorBrush? TintFor(Position pos, Position? checkedKing, Move? lastMove)
    {
        if (Same(pos, checkedKing))
            return new SolidColorBrush(_theme.Check);
        if (Same(pos, _selected))
            return new SolidColorBrush(_theme.Selected);
        if (lastMove is not null && (Same(pos, lastMove.OriginalPosition) || Same(pos, lastMove.NewPosition)))
            return new SolidColorBrush(_theme.LastMove);
        return null;
    }

    private void AddPieceImage(int row, int col, Piece piece)
    {
        var image = new Image
        {
            Source = PieceImages.Get(piece),
            Width = _square,
            Height = _square,
            IsHitTestVisible = false,
        };
        Canvas.SetLeft(image, col * _square);
        Canvas.SetTop(image, row * _square);
        PieceCanvas.Children.Add(image);
        _pieceImages[row, col] = image;
    }

    // --- coordinates -----------------------------------------------------

    private Position ToPosition(int row, int col) => Flipped
        ? new Position((short)(Files - 1 - col), (short)row)
        : new Position((short)col, (short)(Files - 1 - row));

    private (int Row, int Col) ToDisplay(Position p) => Flipped
        ? (p.Y, Files - 1 - p.X)
        : (Files - 1 - p.Y, (int)p.X);

    private static bool Same(Position a, Position? b) => b.HasValue && a.X == b.Value.X && a.Y == b.Value.Y;

    // --- menu ------------------------------------------------------------

    private void OnRightTapped(object sender, RightTappedRoutedEventArgs e)
    {
        Menu.ShowAt(Layers, e.GetPosition(Layers));
        e.Handled = true;
    }

    private void BuildMenu()
    {
        var newGame = new MenuFlyoutItem { Text = "New game" };
        newGame.Click += (_, _) => Reset();
        Menu.Items.Add(newGame);

        var undo = new MenuFlyoutItem { Text = "Undo move" };
        undo.Click += (_, _) => UndoLastMove();
        Menu.Items.Add(undo);

        var flip = new MenuFlyoutItem { Text = "Flip board" };
        flip.Click += (_, _) => Flip();
        Menu.Items.Add(flip);

        Menu.Items.Add(new MenuFlyoutSeparator());

        var themes = new MenuFlyoutItem { Text = "Board colours…" };
        themes.Click += (_, _) => ShowPalettePicker();
        Menu.Items.Add(themes);

        // PGN lives on the hosting page: the tags need the clock and the opponent's
        // rating, and the board deliberately knows about neither.
    }

    // --- palette picker ---------------------------------------------------

    private const double SwatchSize = 26.0;

    /// <summary>
    /// Every palette at once, as a grid that mirrors the one they were generated on:
    /// a row per hue family, and nine columns of chroma x shade.
    ///
    /// A submenu was the obvious thing and the wrong one. A hundred and eleven names in a
    /// list is a list you read; laid out as swatches it is a thing you glance at, and the
    /// names stop having to carry any colour vocabulary at all.
    /// </summary>
    private void ShowPalettePicker()
    {
        var grid = new Grid { RowSpacing = 4, ColumnSpacing = 4, Padding = new Thickness(4) };

        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        for (var i = 0; i < 9; i++)
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(SwatchSize) });

        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        foreach (var _ in BoardTheme.Families)
            grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(SwatchSize) });

        // Header: one label per chroma tier, spanning its three shades.
        var tiers = new[] { Chroma.Muted, Chroma.Soft, Chroma.Rich };
        for (var t = 0; t < tiers.Length; t++)
        {
            var label = new TextBlock
            {
                Text = tiers[t].ToString(),
                FontSize = 11,
                Opacity = 0.7,
                HorizontalAlignment = HorizontalAlignment.Center,
            };
            Grid.SetRow(label, 0);
            Grid.SetColumn(label, 1 + (t * 3));
            Grid.SetColumnSpan(label, 3);
            grid.Children.Add(label);
        }

        var shades = new[] { Shade.Dark, Shade.Mid, Shade.Light };
        var families = BoardTheme.Families;

        for (var r = 0; r < families.Count; r++)
        {
            var family = families[r];

            var name = new TextBlock
            {
                Text = family,
                FontSize = 12,
                VerticalAlignment = VerticalAlignment.Center,
                Margin = new Thickness(0, 0, 4, 0),
            };
            Grid.SetRow(name, r + 1);
            Grid.SetColumn(name, 0);
            grid.Children.Add(name);

            foreach (var theme in BoardTheme.InFamily(family))
            {
                // Greys have no chroma tier, so they sit under Muted — where the eye
                // already expects the least colourful column.
                var tierIndex = theme.Chroma is Chroma.None
                    ? 0
                    : Array.IndexOf(tiers, theme.Chroma);
                var column = 1 + (tierIndex * 3) + Array.IndexOf(shades, theme.Shade);

                var swatch = BuildSwatch(theme);
                Grid.SetRow(swatch, r + 1);
                Grid.SetColumn(swatch, column);
                grid.Children.Add(swatch);
            }
        }

        var custom = new Button
        {
            Content = "Custom…",
            HorizontalAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(0, 6, 0, 0),
        };

        var panel = new StackPanel();
        panel.Children.Add(grid);
        panel.Children.Add(custom);

        var flyout = new Flyout
        {
            Content = panel,
            Placement = Microsoft.UI.Xaml.Controls.Primitives.FlyoutPlacementMode.Full,
        };

        foreach (var child in grid.Children)
            if (child is Button b)
                b.Click += (_, _) => flyout.Hide();

        custom.Click += async (_, _) =>
        {
            flyout.Hide();
            await ShowCustomColourPicker();
        };

        flyout.ShowAt(Layers);
    }

    /// <summary>
    /// Pick any dark square at all. The grid is a shortlist, not a limit — the whole
    /// palette is derived from this one colour, so there is nothing to enumerate.
    ///
    /// The tool's usable band is advisory here rather than enforced: a board outside it
    /// says so, but nobody is stopped from choosing it. Refusing a colour outright would
    /// be a strange thing for a picker to do.
    /// </summary>
    private async Task ShowCustomColourPicker()
    {
        var picker = new ColorPicker
        {
            Color = _theme.Dark,
            IsAlphaEnabled = false,
            IsHexInputVisible = true,
            IsColorSliderVisible = true,
        };

        var warning = new TextBlock
        {
            FontSize = 12,
            Opacity = 0.8,
            Margin = new Thickness(0, 6, 0, 0),
            TextWrapping = TextWrapping.Wrap,
        };

        void Describe(Windows.UI.Color c)
        {
            var candidate = BoardTheme.FromCustom(Pack(c));
            warning.Text = candidate.Status switch
            {
                PaletteStatus.TooDark =>
                    $"Very dark (luminance {candidate.DarkLuminance:F2}). Pieces may be hard to read.",
                PaletteStatus.TooLight =>
                    $"Very light (luminance {candidate.DarkLuminance:F2}). The two square colours will be close.",
                _ => $"Luminance {candidate.DarkLuminance:F2} — comfortable.",
            };
        }

        Describe(picker.Color);
        picker.ColorChanged += (_, e) => Describe(e.NewColor);

        var body = new StackPanel();
        body.Children.Add(picker);
        body.Children.Add(warning);

        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = "Board colour",
            Content = body,
            PrimaryButtonText = "Use it",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
        };

        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            Theme = BoardTheme.FromCustom(Pack(picker.Color));
    }

    private static uint Pack(Windows.UI.Color c) =>
        ((uint)c.R << 16) | ((uint)c.G << 8) | c.B;

    /// <summary>
    /// One palette as a miniature board: two dark squares, two light, ringed in the
    /// border colour, so the swatch shows all three derived colours and their contrast
    /// rather than just the one that was authored.
    /// </summary>
    private Button BuildSwatch(BoardTheme theme)
    {
        var quads = new Grid();
        quads.ColumnDefinitions.Add(new ColumnDefinition());
        quads.ColumnDefinitions.Add(new ColumnDefinition());
        quads.RowDefinitions.Add(new RowDefinition());
        quads.RowDefinitions.Add(new RowDefinition());

        var light = new SolidColorBrush(theme.Light);
        var dark = new SolidColorBrush(theme.Dark);

        for (var i = 0; i < 4; i++)
        {
            var row = i / 2;
            var col = i % 2;
            var cell = new Rectangle { Fill = (row + col) % 2 == 0 ? light : dark };
            Grid.SetRow(cell, row);
            Grid.SetColumn(cell, col);
            quads.Children.Add(cell);
        }

        var selected = theme.Name == _theme.Name;

        var button = new Button
        {
            Padding = new Thickness(0),
            MinWidth = 0,
            MinHeight = 0,
            Width = SwatchSize,
            Height = SwatchSize,
            CornerRadius = new CornerRadius(2),
            BorderThickness = new Thickness(selected ? 2 : 1),
            BorderBrush = new SolidColorBrush(selected ? BoardTheme.Coordinate : theme.Border),
            Background = new SolidColorBrush(theme.Border),
            Content = quads,
        };

        ToolTipService.SetToolTip(button, theme.Name);
        button.Click += (_, _) => Theme = theme;
        return button;
    }

    /// <summary>A coordinate glyph plus the box that centres it in the border.</summary>
    private sealed record CoordLabel(Grid Box, TextBlock Text);
}
