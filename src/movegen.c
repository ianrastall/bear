/****************************************************************************
 * File: movegen.c
 ****************************************************************************/
/*
   Description:
   - Implementation of move generation functionality.
   - Generates pseudo-legal moves for pawns, knights, bishops, rooks,
     queens, kings, including special moves (castling, en passant, promotions).
   - Can return only captures via GenerateCaptures().
   - IsSquareAttacked checks if a given square is attacked by a specified side.

   Notes:
   - Full legality checks (king not in check after move) are typically done
     in MakeMove/UndoMove or a separate legality filter.
*/

#include "movegen.h"   /* For Move struct and prototypes */
#include "defs.h"      /* For piece codes, BOARD_SIZE, etc. */
#include <stddef.h>    /* For NULL if needed */

/* Offsets for piece movement */
static const int KnightOffsets[8] = { -21, -19, -12, -8, 8, 12, 19, 21 };
static const int KingOffsets[8]   = { -9, -11,  9, 11, -10, 10, -1, 1 };
static const int RookOffsets[4]   = { -10, 10, -1, 1 };
static const int BishopOffsets[4] = { -9, -11,  9, 11 };

/* Quickly check if a mailbox index is on the board [0..BOARD_SIZE-1] */
static inline bool IsOnBoard(int sq) {
    return (sq >= 0 && sq < BOARD_SIZE);
}

/* Determine piece color by its code (returns WHITE, BLACK, or BOTH if EMPTY) */
static inline int PieceColor(int piece) {
    if (piece >= W_PAWN && piece <= W_KING)  return WHITE;
    if (piece >= B_PAWN && piece <= B_KING)  return BLACK;
    return BOTH; /* e.g. EMPTY */
}

/* Helper to store a newly generated move into moveList */
static inline void AddMove(
    int from, int to, int captured, int promoted, int flag,
    Move* moveList, int* moveCount
) {
    Move* m = &moveList[*moveCount];
    m->from     = from;
    m->to       = to;
    m->captured = captured;
    m->promoted = promoted;
    m->flag     = flag;
    (*moveCount)++;
}

/*
   PawnMoves:
   - Generates pawn moves for the side to move, both captures and non-captures.
   - capturesOnly = true filters out non-capturing moves except en passant.
*/
static void PawnMoves(const Board* b, Move* moveList, int* moveCount, bool capturesOnly) {
    int side = b->side;
    int pawn = (side == WHITE) ? W_PAWN : B_PAWN;
    int rankDirection = (side == WHITE) ? 10 : -10;

    int leftCapture  = (side == WHITE) ? 9 : -11;
    int rightCapture = (side == WHITE) ? 11 : -9;

    for (int sq = 0; sq < BOARD_SIZE; sq++) {
        if (b->pieces[sq] == pawn) {
            int from = sq;

            /* 1) Single push forward (non-captures) if not capturesOnly */
            if (!capturesOnly) {
                int forwardSq = sq + rankDirection;
                if (IsOnBoard(forwardSq) && (b->pieces[forwardSq] == EMPTY)) {
                    int rank = (forwardSq / 10) - 2;
                    bool isPromotion = false;
                    if ((side == WHITE && rank == 7) || (side == BLACK && rank == 0)) {
                        isPromotion = true;
                    }
                    if (isPromotion) {
                        /* Promote to Q, R, B, N */
                        AddMove(from, forwardSq, EMPTY,
                                (side == WHITE) ? W_QUEEN : B_QUEEN,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, forwardSq, EMPTY,
                                (side == WHITE) ? W_ROOK  : B_ROOK,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, forwardSq, EMPTY,
                                (side == WHITE) ? W_BISHOP: B_BISHOP,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, forwardSq, EMPTY,
                                (side == WHITE) ? W_KNIGHT: B_KNIGHT,
                                (1 << 3), moveList, moveCount);
                    } else {
                        /* Normal pawn push */
                        AddMove(from, forwardSq, EMPTY, EMPTY, 0, moveList, moveCount);

                        /* 2) Double push if on initial rank and forwardSq is empty */
                        bool onInitialRank = false;
                        if (side == WHITE) {
                            /* White initial rank => row=3 => (from / 10) - 2 == 1 => from in [31..38] */
                            if (((from / 10) - 2) == 1) {
                                onInitialRank = true;
                            }
                        } else {
                            /* Black initial rank => row=8 => (from / 10) - 2 == 6 => from in [81..88] */
                            if (((from / 10) - 2) == 6) {
                                onInitialRank = true;
                            }
                        }
                        if (onInitialRank) {
                            int doubleForward = forwardSq + rankDirection;
                            if (IsOnBoard(doubleForward) && b->pieces[doubleForward] == EMPTY) {
                                AddMove(from, doubleForward, EMPTY, EMPTY, (1 << 1), moveList, moveCount);
                            }
                        }
                    }
                }
            }

            /* 3) Captures (left/right diagonal), including en passant */
            int captureLeft  = sq + leftCapture;
            int captureRight = sq + rightCapture;

            /* Left capture */
            if (IsOnBoard(captureLeft)) {
                int capturedPiece = b->pieces[captureLeft];
                if (capturedPiece != EMPTY && PieceColor(capturedPiece) != side) {
                    int rank = (captureLeft / 10) - 2;
                    bool isPromotion = false;
                    if ((side == WHITE && rank == 7) || (side == BLACK && rank == 0)) {
                        /* Promotion capture */
                        AddMove(from, captureLeft, capturedPiece,
                                (side == WHITE) ? W_QUEEN : B_QUEEN,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, captureLeft, capturedPiece,
                                (side == WHITE) ? W_ROOK  : B_ROOK,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, captureLeft, capturedPiece,
                                (side == WHITE) ? W_BISHOP: B_BISHOP,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, captureLeft, capturedPiece,
                                (side == WHITE) ? W_KNIGHT: B_KNIGHT,
                                (1 << 3), moveList, moveCount);
                    } else {
                        /* Normal capture */
                        AddMove(from, captureLeft, capturedPiece, EMPTY, 0, moveList, moveCount);
                    }
                }
                /* En passant capture */
                if (captureLeft == b->enPas) {
                    AddMove(from, captureLeft,
                            (side == WHITE) ? B_PAWN : W_PAWN, 
                            EMPTY, (1 << 0), moveList, moveCount);
                }
            }

            /* Right capture */
            if (IsOnBoard(captureRight)) {
                int capturedPiece = b->pieces[captureRight];
                if (capturedPiece != EMPTY && PieceColor(capturedPiece) != side) {
                    int rank = (captureRight / 10) - 2;
                    bool isPromotion = false;
                    if ((side == WHITE && rank == 7) || (side == BLACK && rank == 0)) {
                        /* Promotion capture */
                        AddMove(from, captureRight, capturedPiece,
                                (side == WHITE) ? W_QUEEN : B_QUEEN,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, captureRight, capturedPiece,
                                (side == WHITE) ? W_ROOK  : B_ROOK,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, captureRight, capturedPiece,
                                (side == WHITE) ? W_BISHOP: B_BISHOP,
                                (1 << 3), moveList, moveCount);
                        AddMove(from, captureRight, capturedPiece,
                                (side == WHITE) ? W_KNIGHT: B_KNIGHT,
                                (1 << 3), moveList, moveCount);
                    } else {
                        /* Normal capture */
                        AddMove(from, captureRight, capturedPiece, EMPTY, 0, moveList, moveCount);
                    }
                }
                /* En passant capture */
                if (captureRight == b->enPas) {
                    AddMove(from, captureRight,
                            (side == WHITE) ? B_PAWN : W_PAWN, 
                            EMPTY, (1 << 0), moveList, moveCount);
                }
            }
        }
    }
}

/*
   GenerateSlidingMoves:
   - For rooks, bishops, queens. We go in each direction until off-board
     or hitting a piece. If it's an enemy piece, we capture and stop.
*/
static void GenerateSlidingMoves(
    const Board* b, Move* moveList, int* moveCount,
    int piece, const int* directions, int dirCount, bool capturesOnly
) {
    for (int sq = 0; sq < BOARD_SIZE; sq++) {
        if (b->pieces[sq] == piece) {
            int from = sq;
            for (int i = 0; i < dirCount; i++) {
                int dir = directions[i];
                int tsq = sq + dir;
                while (IsOnBoard(tsq)) {
                    int targetPiece = b->pieces[tsq];
                    if (targetPiece == EMPTY) {
                        /* Add non-captures only if capturesOnly == false */
                        if (!capturesOnly) {
                            AddMove(from, tsq, EMPTY, EMPTY, 0, moveList, moveCount);
                        }
                    } else {
                        /* Occupied square */
                        if (PieceColor(targetPiece) != PieceColor(piece)) {
                            /* Enemy piece => capture */
                            AddMove(from, tsq, targetPiece, EMPTY, 0, moveList, moveCount);
                        }
                        break; /* Stop in any case */
                    }
                    tsq += dir;
                }
            }
        }
    }
}

/*
   GenerateLeaperMoves:
   - For knights, kings. Each offset is checked once. 
   - If capturesOnly == true, we skip empty squares.
*/
static void GenerateLeaperMoves(
    const Board* b, Move* moveList, int* moveCount,
    int piece, const int* offsets, int offsetCount, bool capturesOnly
) {
    for (int sq = 0; sq < BOARD_SIZE; sq++) {
        if (b->pieces[sq] == piece) {
            int from = sq;
            for (int i = 0; i < offsetCount; i++) {
                int tsq = sq + offsets[i];
                if (IsOnBoard(tsq)) {
                    int targetPiece = b->pieces[tsq];
                    if (targetPiece == EMPTY) {
                        if (!capturesOnly) {
                            AddMove(from, tsq, EMPTY, EMPTY, 0, moveList, moveCount);
                        }
                    } else {
                        /* Capture if enemy */
                        if (PieceColor(targetPiece) != PieceColor(piece)) {
                            AddMove(from, tsq, targetPiece, EMPTY, 0, moveList, moveCount);
                        }
                    }
                }
            }
        }
    }
}

/*
   GenerateCastling:
   - Adds castling moves if available. Currently a placeholder that
     doesn't do thorough checks for check or blockages. 
*/
static void GenerateCastling(
    const Board* b, Move* moveList, int* moveCount, bool capturesOnly
) {
    if (capturesOnly) return; /* Can't capture with castling */

    /* Example skeleton (not fully implemented) */
    /*
    if (b->side == WHITE) {
        if (b->castlePerm & WKCA) {
            // Check squares e1..g1 are free, not attacked, etc.
            // If valid, AddMove(...).
        }
        if (b->castlePerm & WQCA) {
            // Check squares e1..c1 are free, not attacked, etc.
        }
    } else {
        if (b->castlePerm & BKCA) {
            // ...
        }
        if (b->castlePerm & BQCA) {
            // ...
        }
    }
    */
}

/*
   GenerateAllMoves:
   - Generates all pseudo-legal moves for the side to move (captures, non-captures, castling).
   - Returns the total number of moves stored in moveList.
*/
int GenerateAllMoves(const Board* b, Move* moveList)
{
    int moveCount = 0;

    /* 1) Pawn moves (captures + non-captures) */
    PawnMoves(b, moveList, &moveCount, false);

    /* 2) Other pieces */
    if (b->side == WHITE) {
        GenerateLeaperMoves(b, moveList, &moveCount, W_KNIGHT, KnightOffsets, 8, false);
        GenerateSlidingMoves(b, moveList, &moveCount, W_BISHOP, BishopOffsets, 4, false);
        GenerateSlidingMoves(b, moveList, &moveCount, W_ROOK,   RookOffsets,   4, false);
        /* A queen is basically bishop + rook moves */
        GenerateSlidingMoves(b, moveList, &moveCount, W_QUEEN,  BishopOffsets, 4, false);
        GenerateSlidingMoves(b, moveList, &moveCount, W_QUEEN,  RookOffsets,   4, false);
        GenerateLeaperMoves(b, moveList, &moveCount, W_KING,   KingOffsets,    8, false);
    } else {
        GenerateLeaperMoves(b, moveList, &moveCount, B_KNIGHT, KnightOffsets, 8, false);
        GenerateSlidingMoves(b, moveList, &moveCount, B_BISHOP, BishopOffsets, 4, false);
        GenerateSlidingMoves(b, moveList, &moveCount, B_ROOK,   RookOffsets,   4, false);
        GenerateSlidingMoves(b, moveList, &moveCount, B_QUEEN,  BishopOffsets, 4, false);
        GenerateSlidingMoves(b, moveList, &moveCount, B_QUEEN,  RookOffsets,   4, false);
        GenerateLeaperMoves(b, moveList, &moveCount, B_KING,   KingOffsets,    8, false);
    }

    /* 3) Castling */
    GenerateCastling(b, moveList, &moveCount, false);

    return moveCount;
}

/*
   GenerateCaptures:
   - Generates only capturing moves (including en passant and promotion captures).
   - Returns the number of capture moves in moveList.
*/
int GenerateCaptures(const Board* b, Move* moveList)
{
    int moveCount = 0;

    /* Pawn captures (including en passant) */
    PawnMoves(b, moveList, &moveCount, true);

    /* Leapers & sliders, captures only */
    if (b->side == WHITE) {
        GenerateLeaperMoves(b, moveList, &moveCount, W_KNIGHT, KnightOffsets, 8, true);
        GenerateSlidingMoves(b, moveList, &moveCount, W_BISHOP, BishopOffsets, 4, true);
        GenerateSlidingMoves(b, moveList, &moveCount, W_ROOK,   RookOffsets,   4, true);
        GenerateSlidingMoves(b, moveList, &moveCount, W_QUEEN,  BishopOffsets, 4, true);
        GenerateSlidingMoves(b, moveList, &moveCount, W_QUEEN,  RookOffsets,   4, true);
        GenerateLeaperMoves(b, moveList, &moveCount, W_KING,   KingOffsets,    8, true);
    } else {
        GenerateLeaperMoves(b, moveList, &moveCount, B_KNIGHT, KnightOffsets, 8, true);
        GenerateSlidingMoves(b, moveList, &moveCount, B_BISHOP, BishopOffsets, 4, true);
        GenerateSlidingMoves(b, moveList, &moveCount, B_ROOK,   RookOffsets,   4, true);
        GenerateSlidingMoves(b, moveList, &moveCount, B_QUEEN,  BishopOffsets, 4, true);
        GenerateSlidingMoves(b, moveList, &moveCount, B_QUEEN,  RookOffsets,   4, true);
        GenerateLeaperMoves(b, moveList, &moveCount, B_KING,   KingOffsets,    8, true);
    }

    return moveCount;
}

/*
   IsSquareAttacked:
   - Checks if 'square' is attacked by 'side'.
   - Looks for pawn, knight, bishop, rook, queen, and king attacks.
*/
bool IsSquareAttacked(const Board* b, int square, int side)
{
    /* 1) Pawn attacks */
    if (side == WHITE) {
        int left = square - 9;
        int right= square - 11;
        if (IsOnBoard(left)  && b->pieces[left]  == W_PAWN) return true;
        if (IsOnBoard(right) && b->pieces[right] == W_PAWN) return true;
    } else {
        int left = square + 9;
        int right= square + 11;
        if (IsOnBoard(left)  && b->pieces[left]  == B_PAWN) return true;
        if (IsOnBoard(right) && b->pieces[right] == B_PAWN) return true;
    }

    /* 2) Knight attacks */
    for (int i = 0; i < 8; i++) {
        int sqKnight = square + KnightOffsets[i];
        if (IsOnBoard(sqKnight)) {
            int piece = b->pieces[sqKnight];
            if (side == WHITE && piece == W_KNIGHT) return true;
            if (side == BLACK && piece == B_KNIGHT) return true;
        }
    }

    /* 3) Bishop/Queen (diagonals) */
    for (int i = 0; i < 4; i++) {
        int dir = BishopOffsets[i];
        int tsq = square + dir;
        while (IsOnBoard(tsq)) {
            int piece = b->pieces[tsq];
            if (piece != EMPTY) {
                if ((side == WHITE && (piece == W_BISHOP || piece == W_QUEEN)) ||
                    (side == BLACK && (piece == B_BISHOP || piece == B_QUEEN))) {
                    return true;
                }
                break; /* Blocked */
            }
            tsq += dir;
        }
    }

    /* 4) Rook/Queen (straight lines) */
    for (int i = 0; i < 4; i++) {
        int dir = RookOffsets[i];
        int tsq = square + dir;
        while (IsOnBoard(tsq)) {
            int piece = b->pieces[tsq];
            if (piece != EMPTY) {
                if ((side == WHITE && (piece == W_ROOK || piece == W_QUEEN)) ||
                    (side == BLACK && (piece == B_ROOK || piece == B_QUEEN))) {
                    return true;
                }
                break; /* Blocked */
            }
            tsq += dir;
        }
    }

    /* 5) King attacks (adjacent squares) */
    for (int i = 0; i < 8; i++) {
        int sqKing = square + KingOffsets[i];
        if (IsOnBoard(sqKing)) {
            int piece = b->pieces[sqKing];
            if ((side == WHITE && piece == W_KING) ||
                (side == BLACK && piece == B_KING)) {
                return true;
            }
        }
    }

    /* Not attacked if we reach here */
    return false;
}
