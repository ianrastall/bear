using Chess;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Storage.Streams;

namespace Bear.App.Theming;

/// <summary>
/// Loads the Merida piece SVGs. They are compiled in as embedded resources rather than
/// shipped as loose Content: the app publishes as a self-extracting single exe, where
/// ms-appx:/// asset resolution is not dependable. An embedded stream always works.
/// </summary>
internal static class PieceImages
{
    private const string ResourcePrefix = "Bear.App.Assets.Pieces.cburnett.";

    /// <summary>Rasterisation target. Generous so pieces stay crisp on a large board.</summary>
    private const int RasterPx = 320;

    private static readonly Dictionary<string, SvgImageSource> Cache = [];

    /// <summary>Load all twelve pieces once. Safe to call repeatedly.</summary>
    public static async Task PreloadAsync()
    {
        foreach (var colour in (char[])['w', 'b'])
        {
            foreach (var type in (char[])['K', 'Q', 'R', 'B', 'N', 'P'])
                _ = await GetAsync($"{colour}{type}");
        }
    }

    /// <summary>The image for a piece, e.g. "wK". Returns null if the resource is missing.</summary>
    public static SvgImageSource? Get(Piece piece) => Cache.GetValueOrDefault(KeyFor(piece));

    public static string KeyFor(Piece piece)
        => $"{piece.Color.AsChar}{char.ToUpperInvariant(piece.Type.AsChar)}";

    private static async Task<SvgImageSource?> GetAsync(string key)
    {
        if (Cache.TryGetValue(key, out var cached))
            return cached;

        var assembly = typeof(PieceImages).Assembly;
        await using var resource = assembly.GetManifestResourceStream(ResourcePrefix + key + ".svg");
        if (resource is null)
            return null;

        var bytes = new byte[resource.Length];
        await resource.ReadExactlyAsync(bytes);

        var ras = new InMemoryRandomAccessStream();
        using (var writer = new DataWriter(ras.GetOutputStreamAt(0)))
        {
            writer.WriteBytes(bytes);
            await writer.StoreAsync();
            await writer.FlushAsync();
            writer.DetachStream();
        }
        ras.Seek(0);

        var svg = new SvgImageSource
        {
            RasterizePixelWidth = RasterPx,
            RasterizePixelHeight = RasterPx,
        };
        await svg.SetSourceAsync(ras);

        Cache[key] = svg;
        return svg;
    }
}
