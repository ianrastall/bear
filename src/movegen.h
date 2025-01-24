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
#include "move.h" /* Include move.h to use Move structure */

/* Function prototypes */

/* Generate all pseudo-legal moves (captures, non-captures, castling, etc.) */
int GenerateAllMoves(const Board* b, Move* moveList);

/* Generate captures (including en passant and promotion captures) */
int GenerateCaptures(const Board* b, Move* moveList);

/* Check if a given square is attacked by a particular side */
bool IsSquareAttacked(const Board* b, int square, int side);

#endif /* MOVEGEN_H */
