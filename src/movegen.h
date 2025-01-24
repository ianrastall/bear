/****************************************************************************
 * File: movegen.h
 ****************************************************************************/
/*
   Description:
   - Header for move generation routines.
   - Declarations for generating moves from a given board state (captures vs. all).
   - Utility function to check if a square is attacked.

   Exports:
     1) int GenerateAllMoves(const Board* b, Move* moveList);
     2) int GenerateCaptures(const Board* b, Move* moveList);
     3) bool IsSquareAttacked(const Board* b, int square, int side);

   Data structures:
   - Move: a struct with fields for from, to, captured, promoted, flags.
*/

#ifndef MOVEGEN_H
#define MOVEGEN_H

#include "board.h"

/* 
   Define a simple Move structure with fields for 'from', 'to', 
   captured piece, promoted piece, and any special flags.
   Possible flag bits:
     - (1 << 0) : en passant
     - (1 << 1) : pawn start (2-square move)
     - (1 << 2) : castling
     - (1 << 3) : promotion
*/
typedef struct {
    int from;       /* The source square (120-based indexing) */
    int to;         /* The target square (120-based indexing) */
    int captured;   /* Piece type captured, if any */
    int promoted;   /* Piece type if this move is a promotion, else EMPTY */
    int flag;       /* Bitmask for special move attributes */
} Move;

/* Generate all pseudo-legal moves (including captures, non-captures, castling, etc.) */
int GenerateAllMoves(const Board* b, Move* moveList);

/* Generate captures (including en passant and promotions that capture) */
int GenerateCaptures(const Board* b, Move* moveList);

/* Check if a given square is attacked by a particular side */
bool IsSquareAttacked(const Board* b, int square, int side);

#endif /* MOVEGEN_H */
