namespace Bear.App.Maia;

/// <summary>
/// Turns a position into the 18-channel tensor Maia 2 expects.
///
/// A faithful port of <c>preprocessing</c> and <c>board_to_tensor</c> in
/// <c>maia2/utils.py</c>. It works from a FEN rather than from the rules library's object
/// model, for two reasons: the Python side takes a FEN too, so the two can be compared
/// directly; and it keeps this file free of any dependency on how Bear happens to
/// represent a board today.
///
/// The load-bearing detail is the **mirror**. When Black is to move the Python does
/// <c>chess.Board(fen).mirror()</c> — ranks flipped, colours swapped — so the network only
/// ever sees a White-to-move position. Every move it returns is therefore in mirrored
/// coordinates and has to be flipped back. Miss this and Black plays gibberish while White
/// looks perfect, which is a memorably confusing bug to debug from the board.
/// </summary>
internal static class MaiaEncoder
{
    public const int Channels = 18;
    public const int Size = Channels * 8 * 8;

    /// <summary>0-5 are White P N B R Q K; 6-11 the same for Black.</summary>
    private const int TurnChannel = 12;
    private const int CastlingChannel = 13;   // WK, WQ, BK, BQ
    private const int EnPassantChannel = 17;

    /// <summary>
    /// Encode a FEN. <paramref name="mirrored"/> reports whether the board was flipped,
    /// which the caller needs in order to un-flip the move that comes back.
    /// </summary>
    public static float[] Encode(string fen, out bool mirrored)
    {
        var parts = fen.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length < 4)
            throw new ArgumentException($"Not a FEN: {fen}", nameof(fen));

        mirrored = parts[1] == "b";

        var tensor = new float[Size];

        // --- pieces ---
        // FEN lists rank 8 first; the tensor's row 0 is rank 1, so rank r of the FEN
        // (0 = rank 8) lands at row 7 - r. Mirroring flips that row again and swaps colour.
        var rank = 7;
        var file = 0;
        foreach (var c in parts[0])
        {
            if (c == '/')
            {
                rank--;
                file = 0;
            }
            else if (char.IsDigit(c))
            {
                file += c - '0';
            }
            else
            {
                var isWhite = char.IsUpper(c);
                var type = PieceIndex(char.ToLowerInvariant(c));

                var row = mirrored ? 7 - rank : rank;
                var channel = type + (isWhite != mirrored ? 0 : 6);

                tensor[(channel * 64) + (row * 8) + file] = 1f;
                file++;
            }
        }

        // --- side to move ---
        // After mirroring it is always White's turn, so this plane is always set. It is
        // written out rather than assumed, to stay honest to the Python.
        Fill(tensor, TurnChannel);

        // --- castling rights ---
        // Order is White kingside, White queenside, Black kingside, Black queenside.
        // Mirroring swaps the two colours' rights along with everything else.
        var rights = parts[2];
        var wk = rights.Contains('K');
        var wq = rights.Contains('Q');
        var bk = rights.Contains('k');
        var bq = rights.Contains('q');

        if (mirrored)
        {
            (wk, bk) = (bk, wk);
            (wq, bq) = (bq, wq);
        }

        if (wk) Fill(tensor, CastlingChannel + 0);
        if (wq) Fill(tensor, CastlingChannel + 1);
        if (bk) Fill(tensor, CastlingChannel + 2);
        if (bq) Fill(tensor, CastlingChannel + 3);

        // --- en passant target ---
        if (parts[3] != "-" && parts[3].Length >= 2)
        {
            var epFile = parts[3][0] - 'a';
            var epRank = parts[3][1] - '1';
            if (mirrored)
                epRank = 7 - epRank;

            tensor[(EnPassantChannel * 64) + (epRank * 8) + epFile] = 1f;
        }

        return tensor;
    }

    private static void Fill(float[] tensor, int channel)
    {
        var start = channel * 64;
        for (var i = 0; i < 64; i++)
            tensor[start + i] = 1f;
    }

    private static int PieceIndex(char lower) => lower switch
    {
        'p' => 0,
        'n' => 1,
        'b' => 2,
        'r' => 3,
        'q' => 4,
        'k' => 5,
        _ => throw new ArgumentOutOfRangeException(nameof(lower), lower, "Not a piece"),
    };

    /// <summary>
    /// Flip a UCI move between real and mirrored coordinates. Files are unchanged; ranks
    /// become <c>9 - rank</c>. A promotion suffix is carried through untouched.
    /// </summary>
    public static string MirrorMove(string uci)
    {
        if (uci.Length < 4)
            return uci;

        Span<char> buffer = stackalloc char[uci.Length];
        uci.AsSpan().CopyTo(buffer);

        buffer[1] = (char)('0' + (9 - (uci[1] - '0')));
        buffer[3] = (char)('0' + (9 - (uci[3] - '0')));

        return new string(buffer);
    }

    /// <summary>
    /// The rating bucket index, matching <c>map_to_category</c>: eleven buckets, from
    /// <c>&lt;1100</c> through <c>1900-1999</c> to <c>&gt;=2000</c>.
    /// </summary>
    public static long RatingBucket(int elo) => elo switch
    {
        < 1100 => 0,
        >= 2000 => 10,
        _ => ((elo - 1100) / 100) + 1,
    };
}
