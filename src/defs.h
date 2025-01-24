/****************************************************************************
 * File: defs.h
 ****************************************************************************/

#ifndef DEFS_H
#define DEFS_H

#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <string.h>

/* Board dimensions and mailbox approach */
#define BOARD_SIZE 120

/* Maximum search depth, moves, etc. */
#define MAX_DEPTH 64
#define MAX_MOVES 256

/* Sides */
#define WHITE 0
#define BLACK 1
#define BOTH  2

/* Enumerations for piece IDs (0..12) */
enum {
    EMPTY,
    W_PAWN, W_KNIGHT, W_BISHOP, W_ROOK, W_QUEEN, W_KING,
    B_PAWN, B_KNIGHT, B_BISHOP, B_ROOK, B_QUEEN, B_KING
};

/* Castling rights bit flags (if you want them globally) */
#define WKCA 1  /* White can castle kingside */
#define WQCA 2  /* White can castle queenside */
#define BKCA 4  /* Black can castle kingside */
#define BQCA 8  /* Black can castle queenside */

/* Piece values */
#define VAL_PAWN   100
#define VAL_KNIGHT 320
#define VAL_BISHOP 330
#define VAL_ROOK   500
#define VAL_QUEEN  900
#define VAL_KING   20000

/*
   Example macros for converting (file, rank) <-> 120-based index 
   if needed by other files. You might already have these.
*/
#define FR2SQ(f, r)   (((r) + 2) * 10 + ((f) + 1))
/* #define SQ2RANK(sq) ((sq)/10 - 2)
   #define SQ2FILE(sq) ((sq)%10 - 1)
*/

#endif /* DEFS_H */
