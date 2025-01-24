/****************************************************************************
 * File: evaluate.c
 ****************************************************************************/
/*
   Description:
   - Implementation of classical evaluation heuristics.
   - Compute material balance, plus optional minor heuristics.
   - Possibly use piece-square tables for each piece type.
   - Include bishop pair, rook on open files, king safety, etc.
*/

#include "evaluate.h"
#include "defs.h"  /* Not strictly necessary if board.h already includes defs.h,
                      but it's harmless to have it here for clarity. */

/* 
   Example piece-square tables (PST) for White pieces.
   ----------------------------------------------------
   Typically indexed by 0..63 for each square on an 8x8 board, 
   but we can access them by converting 120-based indices to 0-based 
   rank/file if we prefer. 
*/

/* White Pawn PST */
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

/* White King PST (middle-game oriented example) */
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

/* Converts a 120-based square index to 0..63 for PST lookups. */
static int Sq120ToPSTIndex(int sq120)
{
    /* rank = (sq120 / 10) - 2, file = (sq120 % 10) - 1 */
    int rank = (sq120 / 10) - 2;
    int file = (sq120 % 10) - 1;
    return (rank >= 0 && rank < 8 && file >= 0 && file < 8)
         ? rank * 8 + file  /* 0..63 on-board */
         : -1;              /* indicates off-board */
}

/* Mirror a 0..63 index (for black PST lookups) so black uses the same table reversed. */
static int Mirror64(int index)
{
    int rank = index / 8;
    int file = index % 8;
    return (7 - rank) * 8 + file; 
}

/*
   EvaluatePosition:
   - Basic loop for material + PST scoring.
   - Minor heuristics like bishop pair.
*/
int EvaluatePosition(const Board* b)
{
    int score = 0;
    int whiteBishops = 0;
    int blackBishops = 0;

    /* Scan the mailbox. */
    for (int sq = 0; sq < BOARD_SIZE; sq++) {
        int piece = b->pieces[sq];
        if (piece == EMPTY) {
            continue;
        }

        int pstIndex = Sq120ToPSTIndex(sq);
        /* If off-board, skip. */
        if (pstIndex < 0 || pstIndex >= 64) {
            continue;
        }

        switch (piece) {
            /* White Pieces */
            case W_PAWN:
                score += VAL_PAWN + PawnPST[pstIndex];
                break;
            case W_KNIGHT:
                score += VAL_KNIGHT + KnightPST[pstIndex];
                break;
            case W_BISHOP:
                score += VAL_BISHOP + BishopPST[pstIndex];
                whiteBishops++;
                break;
            case W_ROOK:
                score += VAL_ROOK + RookPST[pstIndex];
                break;
            case W_QUEEN:
                score += VAL_QUEEN + QueenPST[pstIndex];
                break;
            case W_KING:
                score += VAL_KING + KingPST[pstIndex];
                break;

            /* Black Pieces */
            case B_PAWN:
                score -= VAL_PAWN;
                score -= PawnPST[Mirror64(pstIndex)];
                break;
            case B_KNIGHT:
                score -= VAL_KNIGHT;
                score -= KnightPST[Mirror64(pstIndex)];
                break;
            case B_BISHOP:
                score -= VAL_BISHOP;
                score -= BishopPST[Mirror64(pstIndex)];
                blackBishops++;
                break;
            case B_ROOK:
                score -= VAL_ROOK;
                score -= RookPST[Mirror64(pstIndex)];
                break;
            case B_QUEEN:
                score -= VAL_QUEEN;
                score -= QueenPST[Mirror64(pstIndex)];
                break;
            case B_KING:
                score -= VAL_KING;
                score -= KingPST[Mirror64(pstIndex)];
                break;

            default:
                /* Should not happen if piece codes are 0..12 only. */
                break;
        }
    }

    /* Bishop pair bonus example. */
    if (whiteBishops >= 2) {
        score += 30;
    }
    if (blackBishops >= 2) {
        score -= 30;
    }

    /* 
       If you want to factor in side to move:
         if (b->side == BLACK) {
             score = -score;
         }
       That depends on your evaluation philosophy.
    */

    return score;
}
