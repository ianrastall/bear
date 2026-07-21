using Windows.UI;

namespace Bear.App.Theming;

/// <summary>How the dark square's luminance rates for board contrast.</summary>
public enum PaletteStatus
{
    TooDark,
    Ok,
    TooLight,
}

/// <summary>How colourful the board is: the saturation axis of the palette grid.</summary>
public enum Chroma
{
    /// <summary>No hue at all. Only the greys use this.</summary>
    None,
    Muted,
    Soft,
    Rich,
}

/// <summary>How dark the board is: the luminance axis of the palette grid.</summary>
public enum Shade
{
    Dark,
    Mid,
    Light,
}

/// <summary>
/// A board colour scheme. Only the dark square is authored; the border and light square
/// are derived from it, which enforces the ordering <c>Border &lt; Dark &lt; Light</c>.
///
/// The derivation is a port of the Board Colors tool at
/// <see href="https://chessnerd.net/board-colors.html"/>: same hue throughout, lightness
/// stepped down 0.18 and up 0.22 in HSL, and — the part that matters visually —
/// saturation *reduced* as you move away from the dark square (border x0.92, light x0.78),
/// which is what keeps light squares from going lurid.
///
/// Relative luminance is a diagnostic here, not the mechanism: it only classifies the
/// dark square as too dark / OK / too light, exactly as the tool reports it.
///
/// Palettes are not named individually. Each one is a cell in a
/// <see cref="Family"/> x <see cref="Chroma"/> x <see cref="Shade"/> grid, so its
/// <see cref="Name"/> states its own coordinates — "Blue-Soft-Dark" needs no colour
/// vocabulary, where "Cadet Blue" does. See <c>tools/gen_palettes.py</c>, which generates
/// the table below and ports this file's maths so its output is identical.
/// </summary>
public sealed record BoardTheme(
    string Family,
    Chroma Chroma,
    Shade Shade,
    Color Dark,
    Color Light,
    Color Border)
{
    /// <summary>Grid coordinates, which is all the name ever needs to be.</summary>
    public string Name => Chroma is Chroma.None
        ? $"{Family}-{Shade}"
        : $"{Family}-{Chroma}-{Shade}";

    /// <summary>Coordinate text drawn in the border, matching the tool's export.</summary>
    public static Color Coordinate { get; } = ColorMath.FromHex(0xF3F3F3);

    /// <summary>Highlight tints are drawn over squares, so they carry their own alpha.</summary>
    public Color Selected { get; } = Color.FromArgb(0x9A, 0xC8, 0xA9, 0x51);
    public Color LastMove { get; } = Color.FromArgb(0x55, 0xC8, 0xA9, 0x51);
    public Color Check { get; } = Color.FromArgb(0xB0, 0xC0, 0x39, 0x2B);
    public Color Marker { get; } = Color.FromArgb(0x50, 0x10, 0x10, 0x10);

    public double DarkLuminance => ColorMath.Luminance(Dark);

    public double ContrastSpan => ColorMath.Luminance(Light) - ColorMath.Luminance(Border);

    public PaletteStatus Status => DarkLuminance switch
    {
        < 0.11 => PaletteStatus.TooDark,
        > 0.42 => PaletteStatus.TooLight,
        _ => PaletteStatus.Ok,
    };

    public static BoardTheme FromDarkSquare(string family, Chroma chroma, Shade shade, uint darkHex)
    {
        // The tool clamps input saturation to 0.95 before deriving; mirror that so a
        // fully saturated base produces the same dark square in both places.
        var input = ColorMath.RgbToHsl(ColorMath.FromHex(darkHex));
        var dark = ColorMath.HslToRgb(input.H, Math.Clamp(input.S, 0.0, 0.95), input.L);

        var d = ColorMath.RgbToHsl(dark);

        var borderSat = Math.Clamp(d.S * 0.92, 0.0, 0.88);
        var lightSat = Math.Clamp(d.S * 0.78, 0.0, 0.82);

        var borderL = Math.Clamp(d.L - 0.18, 0.06, Math.Max(0.07, d.L - 0.06));
        var lightL = Math.Clamp(d.L + 0.22, Math.Min(0.92, d.L + 0.08), 0.92);

        return new BoardTheme(
            family,
            chroma,
            shade,
            dark,
            ColorMath.HslToRgb(d.H, lightSat, lightL),
            ColorMath.HslToRgb(d.H, borderSat, borderL));
    }

    /// <summary>
    /// Twelve hue families x three chroma tiers x three shade tiers, plus three greys.
    ///
    /// The shade tiers are pinned to *luminance* (0.155 / 0.245 / 0.350), not to HSL
    /// lightness, and that is what makes the labels honest: Amber-Rich-Dark solves to
    /// L 0.34 while Indigo-Rich-Dark solves to L 0.57, and they read as equally dark only
    /// because both sit at the same luminance. Every cell lands inside the tool's usable
    /// band, so none had to be dropped.
    ///
    /// Generated — edit <c>tools/gen_palettes.py</c> and paste, rather than hand-tuning.
    /// </summary>
    public static IReadOnlyList<BoardTheme> All { get; } =
    [
        FromDarkSquare("Red", Chroma.Muted, Shade.Dark, 0x846764),        // lum 0.155
        FromDarkSquare("Red", Chroma.Muted, Shade.Mid, 0x9E817E),         // lum 0.245
        FromDarkSquare("Red", Chroma.Muted, Shade.Light, 0xB29A98),       // lum 0.348
        FromDarkSquare("Red", Chroma.Soft, Shade.Dark, 0x9F5B55),         // lum 0.155
        FromDarkSquare("Red", Chroma.Soft, Shade.Mid, 0xB47873),          // lum 0.244
        FromDarkSquare("Red", Chroma.Soft, Shade.Light, 0xC39590),        // lum 0.351
        FromDarkSquare("Red", Chroma.Rich, Shade.Dark, 0xB54C43),         // lum 0.154
        FromDarkSquare("Red", Chroma.Rich, Shade.Mid, 0xC76F67),          // lum 0.245
        FromDarkSquare("Red", Chroma.Rich, Shade.Light, 0xD38F88),        // lum 0.353
        FromDarkSquare("Orange", Chroma.Muted, Shade.Dark, 0x7C6B5E),     // lum 0.156
        FromDarkSquare("Orange", Chroma.Muted, Shade.Mid, 0x988476),      // lum 0.245
        FromDarkSquare("Orange", Chroma.Muted, Shade.Light, 0xAC9D92),    // lum 0.350
        FromDarkSquare("Orange", Chroma.Soft, Shade.Dark, 0x8B664B),      // lum 0.155
        FromDarkSquare("Orange", Chroma.Soft, Shade.Mid, 0xAA7F60),       // lum 0.246
        FromDarkSquare("Orange", Chroma.Soft, Shade.Light, 0xBB9A81),     // lum 0.353
        FromDarkSquare("Orange", Chroma.Rich, Shade.Dark, 0x996138),      // lum 0.156
        FromDarkSquare("Orange", Chroma.Rich, Shade.Mid, 0xBB7848),       // lum 0.245
        FromDarkSquare("Orange", Chroma.Rich, Shade.Light, 0xCA946F),     // lum 0.349
        FromDarkSquare("Amber", Chroma.Muted, Shade.Dark, 0x756D58),      // lum 0.154
        FromDarkSquare("Amber", Chroma.Muted, Shade.Mid, 0x90876D),       // lum 0.244
        FromDarkSquare("Amber", Chroma.Muted, Shade.Light, 0xA79F8A),     // lum 0.348
        FromDarkSquare("Amber", Chroma.Soft, Shade.Dark, 0x7B6C42),       // lum 0.153
        FromDarkSquare("Amber", Chroma.Soft, Shade.Mid, 0x988752),        // lum 0.246
        FromDarkSquare("Amber", Chroma.Soft, Shade.Light, 0xB09F6B),      // lum 0.351
        FromDarkSquare("Amber", Chroma.Rich, Shade.Dark, 0x806B2F),       // lum 0.153
        FromDarkSquare("Amber", Chroma.Rich, Shade.Mid, 0x9E863B),        // lum 0.246
        FromDarkSquare("Amber", Chroma.Rich, Shade.Light, 0xBA9D46),      // lum 0.350
        FromDarkSquare("Olive", Chroma.Muted, Shade.Dark, 0x6D7055),      // lum 0.155
        FromDarkSquare("Olive", Chroma.Muted, Shade.Mid, 0x868A68),       // lum 0.242
        FromDarkSquare("Olive", Chroma.Muted, Shade.Light, 0x9EA384),     // lum 0.351
        FromDarkSquare("Olive", Chroma.Soft, Shade.Dark, 0x6B723D),       // lum 0.155
        FromDarkSquare("Olive", Chroma.Soft, Shade.Mid, 0x848D4C),        // lum 0.245
        FromDarkSquare("Olive", Chroma.Soft, Shade.Light, 0x9BA659),      // lum 0.350
        FromDarkSquare("Olive", Chroma.Rich, Shade.Dark, 0x69732B),       // lum 0.154
        FromDarkSquare("Olive", Chroma.Rich, Shade.Mid, 0x838F35),        // lum 0.247
        FromDarkSquare("Olive", Chroma.Rich, Shade.Light, 0x99A83E),      // lum 0.351
        FromDarkSquare("Green", Chroma.Muted, Shade.Dark, 0x5A7558),      // lum 0.156
        FromDarkSquare("Green", Chroma.Muted, Shade.Mid, 0x70916D),       // lum 0.248
        FromDarkSquare("Green", Chroma.Muted, Shade.Light, 0x8CA78A),     // lum 0.350
        FromDarkSquare("Green", Chroma.Soft, Shade.Dark, 0x467941),       // lum 0.154
        FromDarkSquare("Green", Chroma.Soft, Shade.Mid, 0x579751),        // lum 0.248
        FromDarkSquare("Green", Chroma.Soft, Shade.Light, 0x6FAF6A),      // lum 0.351
        FromDarkSquare("Green", Chroma.Rich, Shade.Dark, 0x357C2E),       // lum 0.154
        FromDarkSquare("Green", Chroma.Rich, Shade.Mid, 0x419939),        // lum 0.242
        FromDarkSquare("Green", Chroma.Rich, Shade.Light, 0x4CB543),      // lum 0.350
        FromDarkSquare("Teal", Chroma.Muted, Shade.Dark, 0x57746C),       // lum 0.156
        FromDarkSquare("Teal", Chroma.Muted, Shade.Mid, 0x6B8F86),        // lum 0.245
        FromDarkSquare("Teal", Chroma.Muted, Shade.Light, 0x89A69E),      // lum 0.351
        FromDarkSquare("Teal", Chroma.Soft, Shade.Dark, 0x40776A),        // lum 0.153
        FromDarkSquare("Teal", Chroma.Soft, Shade.Mid, 0x509483),         // lum 0.245
        FromDarkSquare("Teal", Chroma.Soft, Shade.Light, 0x66AD9B),       // lum 0.351
        FromDarkSquare("Teal", Chroma.Rich, Shade.Dark, 0x2D7B67),        // lum 0.157
        FromDarkSquare("Teal", Chroma.Rich, Shade.Mid, 0x38977F),         // lum 0.245
        FromDarkSquare("Teal", Chroma.Rich, Shade.Light, 0x42B196),       // lum 0.348
        FromDarkSquare("Cyan", Chroma.Muted, Shade.Dark, 0x597277),       // lum 0.155
        FromDarkSquare("Cyan", Chroma.Muted, Shade.Mid, 0x6F8C92),        // lum 0.242
        FromDarkSquare("Cyan", Chroma.Muted, Shade.Light, 0x8CA4A9),      // lum 0.350
        FromDarkSquare("Cyan", Chroma.Soft, Shade.Dark, 0x45757F),        // lum 0.155
        FromDarkSquare("Cyan", Chroma.Soft, Shade.Mid, 0x54909D),         // lum 0.243
        FromDarkSquare("Cyan", Chroma.Soft, Shade.Light, 0x72A9B3),       // lum 0.352
        FromDarkSquare("Cyan", Chroma.Rich, Shade.Dark, 0x317684),        // lum 0.153
        FromDarkSquare("Cyan", Chroma.Rich, Shade.Mid, 0x3D93A4),         // lum 0.245
        FromDarkSquare("Cyan", Chroma.Rich, Shade.Light, 0x51ACBE),       // lum 0.350
        FromDarkSquare("Blue", Chroma.Muted, Shade.Dark, 0x626F81),       // lum 0.156
        FromDarkSquare("Blue", Chroma.Muted, Shade.Mid, 0x7B899B),        // lum 0.245
        FromDarkSquare("Blue", Chroma.Muted, Shade.Light, 0x96A1B0),      // lum 0.351
        FromDarkSquare("Blue", Chroma.Soft, Shade.Dark, 0x527099),        // lum 0.157
        FromDarkSquare("Blue", Chroma.Soft, Shade.Mid, 0x6E8AB1),         // lum 0.247
        FromDarkSquare("Blue", Chroma.Soft, Shade.Light, 0x8BA1C1),       // lum 0.348
        FromDarkSquare("Blue", Chroma.Rich, Shade.Dark, 0x416EAF),        // lum 0.154
        FromDarkSquare("Blue", Chroma.Rich, Shade.Mid, 0x6089C4),         // lum 0.244
        FromDarkSquare("Blue", Chroma.Rich, Shade.Light, 0x81A2D0),       // lum 0.351
        FromDarkSquare("Indigo", Chroma.Muted, Shade.Dark, 0x6F6A8C),     // lum 0.156
        FromDarkSquare("Indigo", Chroma.Muted, Shade.Mid, 0x8A84A3),      // lum 0.246
        FromDarkSquare("Indigo", Chroma.Muted, Shade.Light, 0xA19CB5),    // lum 0.347
        FromDarkSquare("Indigo", Chroma.Soft, Shade.Dark, 0x7064AC),      // lum 0.155
        FromDarkSquare("Indigo", Chroma.Soft, Shade.Mid, 0x8A80BB),       // lum 0.244
        FromDarkSquare("Indigo", Chroma.Soft, Shade.Light, 0xA29AC9),     // lum 0.350
        FromDarkSquare("Indigo", Chroma.Rich, Shade.Dark, 0x6F5FC4),      // lum 0.155
        FromDarkSquare("Indigo", Chroma.Rich, Shade.Mid, 0x8A7CCF),       // lum 0.243
        FromDarkSquare("Indigo", Chroma.Rich, Shade.Light, 0xA298D9),     // lum 0.351
        FromDarkSquare("Violet", Chroma.Muted, Shade.Dark, 0x7E6687),     // lum 0.157
        FromDarkSquare("Violet", Chroma.Muted, Shade.Mid, 0x97809F),      // lum 0.245
        FromDarkSquare("Violet", Chroma.Muted, Shade.Light, 0xAC9AB2),    // lum 0.351
        FromDarkSquare("Violet", Chroma.Soft, Shade.Dark, 0x9057A2),      // lum 0.154
        FromDarkSquare("Violet", Chroma.Soft, Shade.Mid, 0xA676B6),       // lum 0.244
        FromDarkSquare("Violet", Chroma.Soft, Shade.Light, 0xB892C5),     // lum 0.348
        FromDarkSquare("Violet", Chroma.Rich, Shade.Dark, 0x9F48BB),      // lum 0.156
        FromDarkSquare("Violet", Chroma.Rich, Shade.Mid, 0xB26CC9),       // lum 0.244
        FromDarkSquare("Violet", Chroma.Rich, Shade.Light, 0xC28CD4),     // lum 0.350
        FromDarkSquare("Magenta", Chroma.Muted, Shade.Dark, 0x85647C),    // lum 0.156
        FromDarkSquare("Magenta", Chroma.Muted, Shade.Mid, 0x9E7E96),     // lum 0.244
        FromDarkSquare("Magenta", Chroma.Muted, Shade.Light, 0xB299AB),   // lum 0.352
        FromDarkSquare("Magenta", Chroma.Soft, Shade.Dark, 0x9D548B),     // lum 0.154
        FromDarkSquare("Magenta", Chroma.Soft, Shade.Mid, 0xB473A3),      // lum 0.246
        FromDarkSquare("Magenta", Chroma.Soft, Shade.Light, 0xC490B7),    // lum 0.351
        FromDarkSquare("Magenta", Chroma.Rich, Shade.Dark, 0xB14296),     // lum 0.154
        FromDarkSquare("Magenta", Chroma.Rich, Shade.Mid, 0xC666AE),      // lum 0.246
        FromDarkSquare("Magenta", Chroma.Rich, Shade.Light, 0xD388C0),    // lum 0.353
        FromDarkSquare("Rose", Chroma.Muted, Shade.Dark, 0x856571),       // lum 0.155
        FromDarkSquare("Rose", Chroma.Muted, Shade.Mid, 0x9F808B),        // lum 0.247
        FromDarkSquare("Rose", Chroma.Muted, Shade.Light, 0xB29AA3),      // lum 0.352
        FromDarkSquare("Rose", Chroma.Soft, Shade.Dark, 0xA15772),        // lum 0.156
        FromDarkSquare("Rose", Chroma.Soft, Shade.Mid, 0xB5758D),         // lum 0.245
        FromDarkSquare("Rose", Chroma.Soft, Shade.Light, 0xC492A4),       // lum 0.350
        FromDarkSquare("Rose", Chroma.Rich, Shade.Dark, 0xB9446F),        // lum 0.156
        FromDarkSquare("Rose", Chroma.Rich, Shade.Mid, 0xC86A8C),         // lum 0.245
        FromDarkSquare("Rose", Chroma.Rich, Shade.Light, 0xD48BA5),       // lum 0.352
        FromDarkSquare("Neutral", Chroma.None, Shade.Dark, 0x6D6D6D),     // lum 0.153
        FromDarkSquare("Neutral", Chroma.None, Shade.Mid, 0x878787),      // lum 0.242
        FromDarkSquare("Neutral", Chroma.None, Shade.Light, 0xA0A0A0),    // lum 0.352
    ];

    /// <summary>Hue families in wheel order, for the first level of the picker.</summary>
    public static IReadOnlyList<string> Families { get; } =
        All.Select(t => t.Family).Distinct().ToList();

    /// <summary>The palettes of one family, for the picker's swatch grid.</summary>
    public static IReadOnlyList<BoardTheme> InFamily(string family) =>
        All.Where(t => t.Family == family).ToList();

    /// <summary>Look a palette up by name; used when restoring a saved choice.</summary>
    public static BoardTheme? ByName(string? name) =>
        name is null ? null : All.FirstOrDefault(t => t.Name == name);

    public static BoardTheme Default { get; } =
        ByName("Green-Soft-Mid") ?? All[0];
}
