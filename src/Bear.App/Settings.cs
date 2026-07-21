using System.Text.Json;

namespace Bear.App;

/// <summary>
/// The little that survives a restart, as hand-rolled JSON in
/// <c>%LOCALAPPDATA%\Bear\settings.json</c>.
///
/// No settings framework, and deliberately not <c>ApplicationData.Current</c>, which is
/// unavailable to unpackaged apps. Reading and writing both swallow their exceptions: a
/// corrupt or unreadable settings file is a reason to fall back to defaults, never a
/// reason to fail to start.
///
/// One key, and it stays that way — no rating, no player name, no window geometry.
/// </summary>
internal sealed class Settings
{
    /// <summary>
    /// The chosen palette's <see cref="Theming.BoardTheme.Name"/>, not its colours, so
    /// <c>BoardTheme.All</c> stays the single source of truth and a name that no longer
    /// exists simply falls back to the default.
    /// </summary>
    public string? BoardPalette { get; set; }

    private static readonly JsonSerializerOptions Options = new() { WriteIndented = true };

    private static string FilePath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Bear",
        "settings.json");

    public static Settings Load()
    {
        try
        {
            return File.Exists(FilePath)
                ? JsonSerializer.Deserialize<Settings>(File.ReadAllText(FilePath)) ?? new Settings()
                : new Settings();
        }
        catch
        {
            return new Settings();
        }
    }

    public void Save()
    {
        try
        {
            var path = FilePath;
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            File.WriteAllText(path, JsonSerializer.Serialize(this, Options));
        }
        catch
        {
            // Losing a palette choice is not worth interrupting a game over.
        }
    }
}
