namespace Bear.App;

/// <summary>
/// Every game, appended to a PGN in the player's Documents folder as it finishes.
///
/// This is not a game database and does not become one: no schema, no index, no queries,
/// nothing read back. It is an append-only text file in a format that has always held many
/// games, which is why it costs nothing and implies nothing about who owns it. What the
/// database idea actually dragged in was a *rating* derived from the history — that is out
/// of scope, and without it a stored history is just a record.
///
/// Deliberately **not** written beside the executable. A single-file exe gets run from
/// Downloads, from Program Files, from a share; those are often read-only, and Windows will
/// either fail the write or silently redirect it into a VirtualStore path nobody ever finds.
/// <c>Documents\Bear\</c> is writable, discoverable, and backed up.
///
/// Rotated monthly so the set stays predictable and no single file grows without bound.
/// </summary>
internal static class GameLog
{
    public static string Folder => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        "Bear");

    /// <summary>The file the current month's games go to, e.g. <c>bear-2026-07.pgn</c>.</summary>
    public static string CurrentFile =>
        Path.Combine(Folder, $"bear-{DateTime.Now:yyyy-MM}.pgn");

    /// <summary>
    /// Append one game. Games are separated by a blank line, as the PGN spec asks and as
    /// readers expect.
    ///
    /// Failures are swallowed: a full disk or a redirected Documents folder is a reason to
    /// lose one game's record, never a reason to interrupt play. The caller has no
    /// recovery to offer, so there is nothing useful to report.
    /// </summary>
    public static void Append(string pgn)
    {
        if (string.IsNullOrWhiteSpace(pgn))
            return;

        try
        {
            Directory.CreateDirectory(Folder);

            var path = CurrentFile;
            var separator = File.Exists(path) && new FileInfo(path).Length > 0
                ? Environment.NewLine + Environment.NewLine
                : string.Empty;

            File.AppendAllText(path, separator + pgn.Trim() + Environment.NewLine);
        }
        catch
        {
            // See above: nothing here is worth stopping a game for.
        }
    }
}
