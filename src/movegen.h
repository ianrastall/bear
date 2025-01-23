/****************************************************************************
 * File: movegen.h
 ****************************************************************************/
/*
   Description:
   - Header for move generation routines.
   - Declarations for generating all possible moves from a given board state.
   - Possibly separate captures from non-captures (for move ordering).
   - Provide move validation utilities.

   Functions to declare here:
   1) int GenerateAllMoves(const Board* b, Move* moveList);
   2) int GenerateCaptures(const Board* b, Move* moveList);
   3) bool IsSquareAttacked(const Board* b, int square, int side);

   Data structures:
   - Move might be an integer with encoded fields (from, to, piece, flags),
     or a struct with those fields.
*/

#ifndef MOVEGEN_H
#define MOVEGEN_H

#include "board.h"

/* 
   Define a simple Move structure with fields for 'from', 'to', 
   captured piece, promoted piece, and any special flags.
   Flag bits could represent:
     - 1 << 0 for en passant
     - 1 << 1 for pawn start (2-square move)
     - 1 << 2 for castling
     - 1 << 3 for promotion
   etc.
*/
typedef struct {
    int from;       /* The source square (in 120-based indexing) */
    int to;         /* The target square (in 120-based indexing) */
    int captured;   /* Piece type captured, if any (EMPTY if none) */
    int promoted;   /* Piece type if this move is a promotion (EMPTY if none) */
    int flag;       /* Could be used for en passant, castling, etc. */
} Move;

/* 
   Function prototypes for move generation. 
   The return value is typically the number of moves generated and stored
   into moveList.
*/
int GenerateAllMoves(const Board* b, Move* moveList);
int GenerateCaptures(const Board* b, Move* moveList);
bool IsSquareAttacked(const Board* b, int square, int side);

#endif /* MOVEGEN_H */
