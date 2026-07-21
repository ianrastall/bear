using Windows.UI;

namespace Bear.App.Theming;

/// <summary>
/// HSL conversion plus WCAG relative luminance.
///
/// These are deliberate ports of the routines behind
/// <see href="https://chessnerd.net/board-colors.html"/>, kept faithful (including the
/// rounding behaviour) so that a palette designed in that tool produces identical
/// colours here. Do not "improve" them independently of the tool.
/// </summary>
internal static class ColorMath
{
    /// <summary>Hue, saturation and lightness, each in 0..1.</summary>
    public static (double H, double S, double L) RgbToHsl(Color c)
    {
        double r = c.R / 255.0, g = c.G / 255.0, b = c.B / 255.0;

        var max = Math.Max(r, Math.Max(g, b));
        var min = Math.Min(r, Math.Min(g, b));
        var l = (max + min) / 2.0;
        var d = max - min;

        if (d == 0.0)
            return (0.0, 0.0, l);

        var s = l > 0.5 ? d / (2.0 - max - min) : d / (max + min);

        double h;
        if (max == r)
            h = ((g - b) / d) + (g < b ? 6.0 : 0.0);
        else if (max == g)
            h = ((b - r) / d) + 2.0;
        else
            h = ((r - g) / d) + 4.0;

        return (h / 6.0, s, l);
    }

    public static Color HslToRgb(double h, double s, double l)
    {
        if (s == 0.0)
        {
            var grey = (byte)Math.Round(l * 255.0);
            return Color.FromArgb(0xFF, grey, grey, grey);
        }

        var q = l < 0.5 ? l * (1.0 + s) : l + s - (l * s);
        var p = (2.0 * l) - q;

        return Color.FromArgb(
            0xFF,
            Channel(p, q, h + (1.0 / 3.0)),
            Channel(p, q, h),
            Channel(p, q, h - (1.0 / 3.0)));
    }

    /// <summary>WCAG relative luminance. Used as a diagnostic, not to derive colours.</summary>
    public static double Luminance(Color c)
        => (0.2126 * ToLinear(c.R)) + (0.7152 * ToLinear(c.G)) + (0.0722 * ToLinear(c.B));

    public static Color FromHex(uint rgb)
        => Color.FromArgb(0xFF, (byte)(rgb >> 16), (byte)(rgb >> 8), (byte)rgb);

    private static byte Channel(double p, double q, double t)
    {
        var x = t;
        if (x < 0.0) x += 1.0;
        if (x > 1.0) x -= 1.0;

        double v;
        if (x < 1.0 / 6.0)
            v = p + ((q - p) * 6.0 * x);
        else if (x < 1.0 / 2.0)
            v = q;
        else if (x < 2.0 / 3.0)
            v = p + ((q - p) * ((2.0 / 3.0) - x) * 6.0);
        else
            v = p;

        return (byte)Math.Round(Math.Clamp(v, 0.0, 1.0) * 255.0);
    }

    private static double ToLinear(byte channel)
    {
        var v = channel / 255.0;
        return v <= 0.04045 ? v / 12.92 : Math.Pow((v + 0.055) / 1.055, 2.4);
    }
}
