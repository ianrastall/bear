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
/// Rotation is the player's, not the clock's. Games append to whichever log is already in
/// the folder; take one away — file it, archive it, move it somewhere — and the next game
/// starts a fresh one stamped with that day. So a file is named for when it *began*, and
/// the set is trivially distinguishable without anyone naming anything.
/// </summary>
internal static class GameLog
{
    private const string Pattern = "bear-*.pgn";

    public static string Folder => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
        "Bear");

    /// <summary>
    /// The log in progress, or the name a new one would take. <c>bear-2026-07-21.pgn</c>
    /// sorts chronologically as plain text, so "the newest" is just the last name.
    /// </summary>
    public static string CurrentFile
    {
        get
        {
            var existing = Directory.Exists(Folder)
                ? Directory.EnumerateFiles(Folder, Pattern)
                    .OrderBy(Path.GetFileName, StringComparer.Ordinal)
                    .LastOrDefault()
                : null;

            return existing ?? Path.Combine(Folder, $"bear-{DateTime.Now:yyyy-MM-dd}.pgn");
        }
    }

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
