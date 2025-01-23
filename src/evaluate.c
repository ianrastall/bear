/****************************************************************************
 * File: evaluate.c
 ****************************************************************************/
/*
   Description:
   - Implementation of classical evaluation heuristics.
   - Compute material balance, plus optional minor heuristics.
   - Possibly use piece-square tables for each piece type.
   - Include bishop pair, rook on open files, king safety, etc.

   Implementation details:
   1) Summation of piece values for material.
   2) Positional bonuses or penalties (piece-square tables, bishop pair, etc.).
   3) King safety heuristics, rooks on open files, etc.
*/

#include "evaluate.h"
#include "defs.h"

/* 
   Example piece-square tables (PST) for White pieces.
   ----------------------------------------------------
   Typically indexed by 0..63 for each square on an 8x8 board, 
   but we can access them by converting 120-based indices to 0-based 
   rank/file if we prefer. 
   Values below are just placeholders.
*/

/* White Pawn PST (from White's perspective, rank 1 to rank 8) */
static const int PawnPST[64] = {
      0,   0,   0,   0,   0,   0,   0,   0,
     50,  50,  50,  50,  50,  50,  50,  50,
     10,  10,  20,  30,  30,  20,  10,  10,
      5,   5,  10,  25,  25,  10,   5,   5,
      0,   0,   0,  20,  20,   0,   0,   0,
      5,  -5, -10,   0,   0, -10,  -5,   5,
      5,  10,  10, -20, -20,  10,  10,   5,
      0,   0,   0,   0,   0,   0,   0,   0
};

/* White Knight PST */
static const int KnightPST[64] = {
    -50, -40, -30, -30, -30, -30, -40, -50,
    -40, -20,   0,   0,   0,   0, -20, -40,
    -30,   0,  10,  15,  15,  10,   0, -30,
    -30,   5,  15,  20,  20,  15,   5, -30,
    -30,   0,  15,  20,  20,  15,   0, -30,
    -30,   5,  10,  15,  15,  10,   5, -30,
    -40, -20,   0,   5,   5,   0, -20, -40,
    -50, -40, -30, -30, -30, -30, -40, -50
};

/* White Bishop PST */
static const int BishopPST[64] = {
    -20, -10, -10, -10, -10, -10, -10, -20,
    -10,   5,   0,   0,   0,   0,   5, -10,
    -10,  10,  10,  10,  10,  10,  10, -10,
    -10,   0,  10,  10,  10,  10,   0, -10,
    -10,   5,   5,  10,  10,   5,   5, -10,
    -10,   0,   5,  10,  10,   5,   0, -10,
    -10,   0,   0,   0,   0,   0,   0, -10,
    -20, -10, -10, -10, -10, -10, -10, -20
};

/* White Rook PST */
static const int RookPST[64] = {
     0,   0,   0,   5,   5,   0,   0,   0,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
    -5,   0,   0,   0,   0,   0,   0,  -5,
     5,  10,  10,  10,  10,  10,  10,   5,
     0,   0,   0,   0,   0,   0,   0,   0
};

/* White Queen PST */
static const int QueenPST[64] = {
    -20, -10, -10,  -5,  -5, -10, -10, -20,
    -10,   0,   5,   0,   0,   0,   0, -10,
    -10,   5,   5,   5,   5,   5,   0, -10,
     -5,   0,   5,   5,   5,   5,   0,  -5,
      0,   0,   5,   5,   5,   5,   0,  -5,
    -10,   5,   5,   5,   5,   5,   0, -10,
    -10,   0,   5,   0,   0,   0,   0, -10,
    -20, -10, -10,  -5,  -5, -10, -10, -20
};

/* White King PST (middle-game oriented, for example) */
static const int KingPST[64] = {
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -30, -40, -40, -50, -50, -40, -40, -30,
    -20, -30, -30, -40, -40, -30, -30, -20,
    -10, -20, -20, -20, -20, -20, -20, -10,
     20,  20,   0,   0,   0,   0,  20,  20,
     20,  30,  10,   0,   0,  10,  30,  20
};

/* 
   A helper to convert a 120-based square index to 
   a 0..63 index for piece-square tables. 
*/
static int Sq120ToPSTIndex(int sq120)
{
    /* 
       For sq120, the rank is (sq120 / 10) - 2, 
       the file is (sq120 % 10) - 1. 
    */
    int rank = (sq120 / 10) - 2;
    int file = (sq120 % 10) - 1;
    return rank * 8 + file; /* 0..63 for a valid on-board square */
}

/* 
   A helper that returns the mirrored PST index for black pieces.
   This can be a simple approach: invert the rank so it uses the
   same table from the black perspective, or define separate PSTs
   for black. 
*/
static int Mirror64(int index)
{
    /* index is rank*8 + file */
    int rank = index / 8; 
    int file = index % 8;
    return (7 - rank) * 8 + file; 
}

/*
   EvaluatePosition:
   - Combines multiple heuristics to generate a final score:
     1) Material count (classical piece values).
     2) Piece-square tables for positional scoring.
     3) (Optional) Bishop pair, rooks on open files, king safety, etc.

   Returns a positive score if the position is better for White,
   negative if it is better for Black.
*/
int EvaluatePosition(const Board* b)
{
    int score = 0;
    int sq;

    /* Keep track of piece counts for potential heuristics like bishop pair */
    int whiteBishops = 0;
    int blackBishops = 0;

    /* 
       Basic material + PST scoring loop:
       We traverse all 120 squares. If a square is on the board, 
       we check the piece and add the relevant material + PST values.
    */
    for (sq = 0; sq < BOARD_SIZE; sq++) {
        int piece = b->pieces[sq];
        if (piece == EMPTY) {
            continue;
        }

        /* Convert sq to 0..63 index for PST lookups (if it's valid on-board) */
        int pstIndex = Sq120ToPSTIndex(sq);

        /* Skip if off-board (rank or file might be out of [0..7]) */
        if (pstIndex < 0 || pstIndex >= 64) {
            continue;
        }

        switch (piece) {
            /* White Pieces */
            case W_PAWN:
                score += VAL_PAWN;
                score += PawnPST[pstIndex];
                break;
            case W_KNIGHT:
                score += VAL_KNIGHT;
                score += KnightPST[pstIndex];
                break;
            case W_BISHOP:
                score += VAL_BISHOP;
                score += BishopPST[pstIndex];
                whiteBishops++;
                break;
            case W_ROOK:
                score += VAL_ROOK;
                score += RookPST[pstIndex];
                break;
            case W_QUEEN:
                score += VAL_QUEEN;
                score += QueenPST[pstIndex];
                break;
            case W_KING:
                score += VAL_KING;
                score += KingPST[pstIndex];
                break;

            /* Black Pieces */
            case B_PAWN:
                score -= VAL_PAWN;
                /* Mirror PST for black or define separate black tables */
                score -= PawnPST[Mirror64(pstIndex]];
                break;
            case B_KNIGHT:
                score -= VAL_KNIGHT;
                score -= KnightPST[Mirror64(pstIndex]];
                break;
            case B_BISHOP:
                score -= VAL_BISHOP;
                score -= BishopPST[Mirror64(pstIndex]];
                blackBishops++;
                break;
            case B_ROOK:
                score -= VAL_ROOK;
                score -= RookPST[Mirror64(pstIndex]];
                break;
            case B_QUEEN:
                score -= VAL_QUEEN;
                score -= QueenPST[Mirror64(pstIndex]];
                break;
            case B_KING:
                score -= VAL_KING;
                score -= KingPST[Mirror64(pstIndex]];
                break;

            default:
                /* Should not happen if we only have 0..12 as piece codes */
                break;
        }
    }

    /* 
       Minor heuristics:
       Example 1: Bishop pair bonus.
       If a side has 2 or more bishops, add a small bonus.
       Adjust the value as you see fit, e.g., 30 is a typical bishop pair bonus.
    */
    if (whiteBishops >= 2) {
        score += 30; /* White bishop pair bonus */
    }
    if (blackBishops >= 2) {
        score -= 30; /* Black bishop pair bonus */
    }

    /*
       Example 2: Rook on open file or semi-open file.
       We would need to detect open or semi-open files. 
       For now, just a placeholder to demonstrate:
         - You could track whether a file has no pawns or only enemy pawns, etc.
    */

    /*
       Example 3: King safety. 
       A robust engine might look at pawn shields, or the king's 
       exposure to checks. We skip that for brevity here.
    */

    /*
       Adjusting for side to move:
       ---------------------------------------------------
       - Some engines like to return a perspective-based
         score (where side to move tries to maximize).
         - For example, if it's Black to move, we might invert the sign.
       - Others always keep "White is positive, Black is negative."
         Then, the search routine interprets the score accordingly.
       For now, let's keep it as "White positive, Black negative."
    */
    // if (b->side == BLACK) {
    //     score = -score;
    // }

    return score;
}
