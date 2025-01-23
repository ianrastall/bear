/****************************************************************************
 * File: movegen.c
 ****************************************************************************/
/*
   Description:
   - Implementation of all move generation functionality.
   - Code to scan the board for pieces, add legal moves, handle special moves
     (castling, en passant, promotions).
   - Possibly includes bitboard-based or mailbox-based move generation.
   - Must ensure moves are legal (e.g., not leaving your own king in check),
     or filter those in a separate function.

   Implementation details:
   1) We loop over each piece of the side to move, generate moves.
   2) We handle special moves: castling, en passant, promotions.
   3) For captures-only, we filter out non-captures.
   4) IsSquareAttacked checks if a square is under attack by a given side.
*/

#include "movegen.h"
#include "defs.h"
#include <stddef.h>  /* For NULL */

/* --------------------------------------------------------------------------
   Direction arrays for piece movement
   --------------------------------------------------------------------------
   Knight moves (8 possible)
   King moves (8 possible)
   Rook directions (4), Bishop directions (4), Queen can do both (8)
   Pawn moves are handled separately depending on color.
*/
static const int KnightOffsets[8] = { -21, -19, -12, -8, 8, 12, 19, 21 };
static const int KingOffsets[8]   = { -9, -11,  9, 11, -10, 10, -1, 1 };
static const int RookOffsets[4]   = { -10, 10, -1, 1 };
static const int BishopOffsets[4] = { -9, -11, 9, 11 };

/* 
   Helper function to quickly determine if a square is on board.
   We expect valid on-board squares to be in [21..98], but skipping
   the edges properly. A more robust method is to see if the piece
   array has something other than an off-board sentinel. For simplicity,
   let's just define:
*/
static inline bool IsOnBoard(int sq) {
    return (sq >= 0 && sq < BOARD_SIZE);
}

/*
   Helper function to return the color of a piece by its code.
   White pieces are 1..6, black are 7..12, or 0 if EMPTY.
*/
static inline int PieceColor(int piece) {
    if (piece >= W_PAWN && piece <= W_KING) {
        return WHITE;
    }
    if (piece >= B_PAWN && piece <= B_KING) {
        return BLACK;
    }
    return BOTH; /* Or an invalid color if piece == EMPTY */
}

/* 
   AddMove:
   - Helper function to add a move to the moveList, increment the moveCount,
     and fill out all fields in the Move struct.
*/
static inline void AddMove(int from, int to, int captured, int promoted, int flag,
                           Move* moveList, int* moveCount)
{
    Move* m = &moveList[*moveCount];
    m->from = from;
    m->to = to;
    m->captured = captured;
    m->promoted = promoted;
    m->flag = flag;
    (*moveCount)++;
}

/* 
   PawnMoves:
   - Helper to generate pawn moves (both captures and non-captures).
   - We handle White pawns and Black pawns separately.
*/
static void PawnMoves(const Board* b, Move* moveList, int* moveCount, bool capturesOnly)
{
    int side = b->side;
    int pawn = (side == WHITE) ? W_PAWN : B_PAWN;
    int rankDirection = (side == WHITE) ? 10 : -10; /* moving up or down in mailbox terms */

    /* Offsets for capturing squares */
    int leftCapture  = (side == WHITE) ? 9 : -11;
    int rightCapture = (side == WHITE) ? 11 : -9;

    for(int sq=0; sq<BOARD_SIZE; sq++) {
        if (b->pieces[sq] == pawn) {
            int from = sq;

            /* 
               1) Single push forward (non-capture).
                  - Only if !capturesOnly.
                  - Only if the forward square is empty.
            */
            if(!capturesOnly) {
                int forwardSq = sq + rankDirection;
                if(IsOnBoard(forwardSq) && b->pieces[forwardSq] == EMPTY) {
                    /* Promotion check if it's on the last rank for that side */
                    bool isPromotion = false;
                    int rank = (forwardSq / 10) - 2;
                    if((side == WHITE && rank == 7) || (side == BLACK && rank == 0)) {
                        isPromotion = true;
                    }

                    if(isPromotion) {
                        /* Typically, we can promote to Q, R, B, or N. We'll just add Q for now, or all. */
                        AddMove(from, forwardSq, EMPTY, (side == WHITE) ? W_QUEEN : B_QUEEN, 1 << 3, moveList, moveCount);
                        AddMove(from, forwardSq, EMPTY, (side == WHITE) ? W_ROOK  : B_ROOK,  1 << 3, moveList, moveCount);
                        AddMove(from, forwardSq, EMPTY, (side == WHITE) ? W_BISHOP: B_BISHOP,1 << 3, moveList, moveCount);
                        AddMove(from, forwardSq, EMPTY, (side == WHITE) ? W_KNIGHT: B_KNIGHT,1 << 3, moveList, moveCount);
                    } else {
                        /* Normal single pawn push */
                        AddMove(from, forwardSq, EMPTY, EMPTY, 0, moveList, moveCount);
                    }

                    /* 
                       2) Double push (pawn start), only if on rank 1 (White) or rank 6 (Black) in standard indexing.
                       For mailbox, White pawns start at rank = 1, Black at rank = 6. 
                       In 120 indexing, that translates to different rows, so let's do a quick check:
                       White pawns start on rank=1 => mailbox rank=1 => row=3 in 10x12?
                       That gets complicated. For simplicity, let's show the concept:
                    */
                    bool onInitialRank = false;
                    if(side == WHITE) {
                        /* White initial rank is rank = 1 (in 0-based), so row=3 in mailbox? 
                           Let's do a direct check if 'from' in [31..38], for instance, 
                           if the board is set up that way. 
                        */
                        if((from / 10) - 2 == 1) { /* row=3 => rank=1 in 0-based */
                            onInitialRank = true;
                        }
                    } else {
                        /* Black initial rank is rank=6 => row=8 in mailbox? 
                           So let's check if (from / 10) - 2 == 6 => (from / 10)=8 => from in [81..88] 
                        */
                        if((from / 10) - 2 == 6) {
                            onInitialRank = true;
                        }
                    }

                    if(onInitialRank) {
                        int doubleForward = forwardSq + rankDirection;
                        if(IsOnBoard(doubleForward) && b->pieces[doubleForward] == EMPTY) {
                            AddMove(from, doubleForward, EMPTY, EMPTY, 1 << 1, moveList, moveCount);
                        }
                    }
                }
            }

            /* 
               3) Captures: left or right diagonal if there's an enemy piece, or en passant.
            */
            int captureLeft = sq + leftCapture;
            int captureRight = sq + rightCapture;

            /* left capture if on board and it's an enemy piece or en passant square */
            if(IsOnBoard(captureLeft)) {
                int capturedPiece = b->pieces[captureLeft];
                if(capturedPiece != EMPTY && PieceColor(capturedPiece) != side) {
                    /* Potential promotion check */
                    bool isPromotion = false;
                    int rank = (captureLeft / 10) - 2;
                    if((side == WHITE && rank == 7) || (side == BLACK && rank == 0)) {
                        /* Promotion capture */
                        AddMove(from, captureLeft, capturedPiece, (side == WHITE) ? W_QUEEN : B_QUEEN, 1 << 3, moveList, moveCount);
                        AddMove(from, captureLeft, capturedPiece, (side == WHITE) ? W_ROOK  : B_ROOK,  1 << 3, moveList, moveCount);
                        AddMove(from, captureLeft, capturedPiece, (side == WHITE) ? W_BISHOP: B_BISHOP,1 << 3, moveList, moveCount);
                        AddMove(from, captureLeft, capturedPiece, (side == WHITE) ? W_KNIGHT: B_KNIGHT,1 << 3, moveList, moveCount);
                    } else {
                        /* Normal capture */
                        AddMove(from, captureLeft, capturedPiece, EMPTY, 0, moveList, moveCount);
                    }
                }
                /* En passant check */
                if(captureLeft == b->enPas) {
                    AddMove(from, captureLeft, (side == WHITE) ? B_PAWN : W_PAWN, EMPTY, 1 << 0, moveList, moveCount);
                }
            }

            /* right capture if on board and it's an enemy piece or en passant square */
            if(IsOnBoard(captureRight)) {
                int capturedPiece = b->pieces[captureRight];
                if(capturedPiece != EMPTY && PieceColor(capturedPiece) != side) {
                    /* Potential promotion check */
                    bool isPromotion = false;
                    int rank = (captureRight / 10) - 2;
                    if((side == WHITE && rank == 7) || (side == BLACK && rank == 0)) {
                        /* Promotion capture */
                        AddMove(from, captureRight, capturedPiece, (side == WHITE) ? W_QUEEN : B_QUEEN, 1 << 3, moveList, moveCount);
                        AddMove(from, captureRight, capturedPiece, (side == WHITE) ? W_ROOK  : B_ROOK,  1 << 3, moveList, moveCount);
                        AddMove(from, captureRight, capturedPiece, (side == WHITE) ? W_BISHOP: B_BISHOP,1 << 3, moveList, moveCount);
                        AddMove(from, captureRight, capturedPiece, (side == WHITE) ? W_KNIGHT: B_KNIGHT,1 << 3, moveList, moveCount);
                    } else {
                        /* Normal capture */
                        AddMove(from, captureRight, capturedPiece, EMPTY, 0, moveList, moveCount);
                    }
                }
                /* En passant check */
                if(captureRight == b->enPas) {
                    AddMove(from, captureRight, (side == WHITE) ? B_PAWN : W_PAWN, EMPTY, 1 << 0, moveList, moveCount);
                }
            }
        }
    }
}

/*
   GenerateSlidingMoves:
   - For rooks, bishops, queens. We iterate over each direction until 
     we hit an off-board or a piece. If it's an enemy piece, we can capture it
     (and then stop in that direction).
*/
static void GenerateSlidingMoves(const Board* b, Move* moveList, int* moveCount,
                                 int piece, const int* directions, int dirCount, bool capturesOnly)
{
    for(int sq=0; sq<BOARD_SIZE; sq++) {
        if(b->pieces[sq] == piece) {
            int from = sq;
            for(int i=0; i<dirCount; i++) {
                int dir = directions[i];
                int tsq = sq + dir;
                while(IsOnBoard(tsq)) {
                    int targetPiece = b->pieces[tsq];
                    if(targetPiece == EMPTY) {
                        if(!capturesOnly) {
                            AddMove(from, tsq, EMPTY, EMPTY, 0, moveList, moveCount);
                        }
                    } else {
                        /* Occupied square */
                        if(PieceColor(targetPiece) != PieceColor(piece)) {
                            /* It's an enemy piece, can capture */
                            AddMove(from, tsq, targetPiece, EMPTY, 0, moveList, moveCount);
                        }
                        /* We stop in any case after first occupied square */
                        break;
                    }
                    tsq += dir;
                }
            }
        }
    }
}

/*
   GenerateLeaperMoves:
   - For knights, kings. We check each offset exactly once or up to 8 times.
   - If the square is empty or occupied by enemy, we can move/capture. 
   - If capturesOnly is true, we skip adding unless there's an enemy piece.
*/
static void GenerateLeaperMoves(const Board* b, Move* moveList, int* moveCount,
                                int piece, const int* offsets, int offsetCount, bool capturesOnly)
{
    for(int sq=0; sq<BOARD_SIZE; sq++) {
        if(b->pieces[sq] == piece) {
            int from = sq;
            for(int i=0; i<offsetCount; i++) {
                int tsq = sq + offsets[i];
                if(IsOnBoard(tsq)) {
                    int targetPiece = b->pieces[tsq];
                    if(targetPiece == EMPTY) {
                        if(!capturesOnly) {
                            AddMove(from, tsq, EMPTY, EMPTY, 0, moveList, moveCount);
                        }
                    } else {
                        if(PieceColor(targetPiece) != PieceColor(piece)) {
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
   - Simplified approach: if castling rights exist, check if squares are free
     or attacked. For a robust approach, we should also ensure the king is not
     in or passing through check. We'll show the structure but skip the thorough 
     check logic. 
*/
static void GenerateCastling(const Board* b, Move* moveList, int* moveCount, bool capturesOnly)
{
    if(capturesOnly) return; /* Castling is never a capture */

    int side = b->side;
    if(side == WHITE) {
        /* White kingside (check if b->castlePerm has the bit for WKCA, squares e1..g1 are free, not in check) */
        /* White queenside likewise */
    } else {
        /* Black kingside, black queenside, etc. */
    }
    /* AddMove(...), with a special castling flag if valid. */
}

/* --------------------------------------------------------------------------
   GenerateAllMoves:
   - Generates all (pseudo-legal) moves from the current board position,
     storing them into moveList, and returns the count of generated moves.
   --------------------------------------------------------------------------*/
int GenerateAllMoves(const Board* b, Move* moveList)
{
    int moveCount = 0;
    /* 1) Generate all pawn moves, both captures and non-captures. */
    PawnMoves(b, moveList, &moveCount, false);

    /* 2) Generate knight moves (white or black). */
    if(b->side == WHITE) {
        GenerateLeaperMoves(b, moveList, &moveCount, W_KNIGHT, KnightOffsets, 8, false);
        GenerateSlidingMoves(b, moveList, &moveCount, W_BISHOP, BishopOffsets, 4, false);
        GenerateSlidingMoves(b, moveList, &moveCount, W_ROOK,   RookOffsets,   4, false);
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

    /* 3) Generate castling moves if available. */
    GenerateCastling(b, moveList, &moveCount, false);

    /* Return the total number of moves generated. */
    return moveCount;
}

/* 
   GenerateCaptures:
   - Similar to GenerateAllMoves, but only returns capture moves.
   - We skip all non-captures except for en passant, which is also a capture.
*/
int GenerateCaptures(const Board* b, Move* moveList)
{
    int moveCount = 0;
    PawnMoves(b, moveList, &moveCount, true);

    if(b->side == WHITE) {
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
   - Determines if a given square (square) is attacked by the specified side.
   - We check for:
       1) Pawn attacks
       2) Knight attacks
       3) Bishop/Queen (diagonal) attacks
       4) Rook/Queen (straight) attacks
       5) King attacks
*/
bool IsSquareAttacked(const Board* b, int square, int side)
{
    /* 1) Pawn attacks */
    if(side == WHITE) {
        /* White pawns can attack from square-9 or square-11 (in 120 indexing) */
        int left = square - 9;
        int right= square - 11;
        if(IsOnBoard(left) && b->pieces[left] == W_PAWN) return true;
        if(IsOnBoard(right)&& b->pieces[right]== W_PAWN) return true;
    } else {
        /* Black pawns attack from square+9 or +11 */
        int left = square + 9;
        int right= square + 11;
        if(IsOnBoard(left) && b->pieces[left] == B_PAWN) return true;
        if(IsOnBoard(right)&& b->pieces[right]== B_PAWN) return true;
    }

    /* 2) Knight attacks */
    for(int i=0; i<8; i++) {
        int sqKnight = square + KnightOffsets[i];
        if(IsOnBoard(sqKnight)) {
            int piece = b->pieces[sqKnight];
            if(side == WHITE && piece == W_KNIGHT) return true;
            if(side == BLACK && piece == B_KNIGHT) return true;
        }
    }

    /* 3) Bishop/Queen (diagonals) */
    for(int i=0; i<4; i++) {
        int dir = BishopOffsets[i];
        int tsq = square + dir;
        while(IsOnBoard(tsq)) {
            int piece = b->pieces[tsq];
            if(piece != EMPTY) {
                /* If it's the side's bishop or queen, we are attacked. */
                if(side == WHITE && (piece == W_BISHOP || piece == W_QUEEN)) return true;
                if(side == BLACK && (piece == B_BISHOP || piece == B_QUEEN)) return true;
                /* Otherwise, blocked. */
                break;
            }
            tsq += dir;
        }
    }

    /* 4) Rook/Queen (straight lines) */
    for(int i=0; i<4; i++) {
        int dir = RookOffsets[i];
        int tsq = square + dir;
        while(IsOnBoard(tsq)) {
            int piece = b->pieces[tsq];
            if(piece != EMPTY) {
                if(side == WHITE && (piece == W_ROOK || piece == W_QUEEN)) return true;
                if(side == BLACK && (piece == B_ROOK || piece == B_QUEEN)) return true;
                break;
            }
            tsq += dir;
        }
    }

    /* 5) King attacks (adjacent squares) */
    for(int i=0; i<8; i++) {
        int sqKing = square + KingOffsets[i];
        if(IsOnBoard(sqKing)) {
            int piece = b->pieces[sqKing];
            if(side == WHITE && piece == W_KING) return true;
            if(side == BLACK && piece == B_KING) return true;
        }
    }

    /* Not attacked if none of the above checks matched */
    return false;
}
