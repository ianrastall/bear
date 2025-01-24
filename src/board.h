/****************************************************************************
 * File: board.h
 ****************************************************************************/
/*
   Description:
   - Declares the Board struct and board-related function prototypes.
   - We rely on defs.h for all global constants (BOARD_SIZE, piece codes, etc.).
*/

#ifndef BOARD_H
#define BOARD_H

#include "defs.h"

/*
   Board structure for mailbox 120 representation.
   ------------------------------------------------
   - pieces[]:    array of size BOARD_SIZE holding piece codes (EMPTY..B_KING).
   - side:        which side is to move (WHITE or BLACK).
   - enPas:       120-based index of en passant square, or -1 if none.
   - fiftyMove:   halfmove clock for 50-move rule.
   - ply:         current search depth (if used).
   - hisPly:      number of half-moves so far in the game.
   - castlePerm:  bitmask of castling rights (WKCA, WQCA, BKCA, BQCA).
   - positionKey: a Zobrist-like hash key for the board state.
*/
typedef struct {
    int pieces[BOARD_SIZE];
    int side;
    int enPas;
    int fiftyMove;
    int ply;
    int hisPly;
    int castlePerm;
    unsigned long long positionKey;
} Board;

/* Function prototypes */
void InitBoard(Board* b);
void SetFen(Board* b, const char* fen);
void PrintBoard(const Board* b);

#endif /* BOARD_H */
