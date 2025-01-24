/****************************************************************************
 * File: defs.h
 ****************************************************************************/
/*
   Global definitions and constants for the Bear chess engine.
*/

#ifndef DEFS_H
#define DEFS_H

#include <stdbool.h>

/* Board size in mailbox representation (10x12) */
#define BOARD_SIZE 120

/* Piece codes */
#define EMPTY 0

#define W_PAWN   1
#define W_KNIGHT 2
#define W_BISHOP 3
#define W_ROOK   4
#define W_QUEEN  5
#define W_KING   6

#define B_PAWN   7
#define B_KNIGHT 8
#define B_BISHOP 9
#define B_ROOK   10
#define B_QUEEN  11
#define B_KING   12

/* Side to move */
#define WHITE 0
#define BLACK 1

/* Castling flags */
#define WKCA (1 << 0) /* White Kingside Castling Allowed */
#define WQCA (1 << 1) /* White Queenside Castling Allowed */
#define BKCA (1 << 2) /* Black Kingside Castling Allowed */
#define BQCA (1 << 3) /* Black Queenside Castling Allowed */

/* Evaluation constants */
#define INFINITY 1000000
#define MATE 999900

#endif /* DEFS_H */
