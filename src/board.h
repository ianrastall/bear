/****************************************************************************
 * File: board.h
 ****************************************************************************/
/*
    Description:
    - Header for board-related functions and structures.
    - Manages the board state, piece placements, move execution, and FEN parsing.
*/

#ifndef BOARD_H
#define BOARD_H

#include "defs.h"
#include "move.h" /* Include move.h to use Move structure */
#include <stdbool.h>

/* Constants for castling rights */
#define WKCA (1 << 0) /* White Kingside Castling Allowed */
#define WQCA (1 << 1) /* White Queenside Castling Allowed */
#define BKCA (1 << 2) /* Black Kingside Castling Allowed */
#define BQCA (1 << 3) /* Black Queenside Castling Allowed */

/* Board structure */
typedef struct {
    int pieces[BOARD_SIZE];
    int side;
    int enPas;
    int castlePerm;
    // Add other fields as necessary
} Board;

/* Function prototypes */
void InitBoard(Board* b);
void SetFen(Board* b, const char* fen, bool debugMode);
bool IsMoveLegal(Board* b, Move move); /* Implement legality check */
void MakeMove(Board* b, Move move);    /* Implement making a move */

#endif /* BOARD_H */
