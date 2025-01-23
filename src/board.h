/****************************************************************************
 * File: board.h
 ****************************************************************************/
/*
   Description:
   - Header for board.c, declares structures and functions used to represent
     the chessboard.
   - Typically includes piece placement, side to move, castling rights, etc.
*/

#ifndef BOARD_H
#define BOARD_H

#include "defs.h"

/*
   The Board structure for a mailbox 120 representation.
   -------------------------------------------------------------------------
   pieces[]: Holds piece codes for each of the 120 squares in the mailbox
             layout. Indices 0..119 are valid squares, but note that many
             will be off-board borders if we use a 10x12 layout.

   side: Current side to move (WHITE or BLACK).

   enPas: Holds the 120-based index of the en passant square or -1 if none
          is available.

   fiftyMove: The halfmove clock used for the 50-move rule.

   ply: The current search depth in the engine’s search tree (incremented
        for each half-move in the search).

   hisPly: The total number of half-moves made in the game so far (not just
           in the search).

   castlePerm: A bitmask representing castling rights. 
               For example, using bits:
                   1 = White can castle kingside
                   2 = White can castle queenside
                   4 = Black can castle kingside
                   8 = Black can castle queenside

   positionKey: A Zobrist hash key representing the current board position.
                This is used for transposition tables and repetition checks.

   -------------------------------------------------------------------------
   In a more advanced engine, you might also store arrays or counts for:
   - pieceCount[13]: number of each type of piece on board
   - pieceList[13][10]: the mailbox indices where each piece is located
   - bitboards for each piece type, or for occupancy, etc.

   But for now, we keep the structure simple.
*/

typedef struct {
    int pieces[BOARD_SIZE];             /* Array to hold piece codes (see defs.h) */
    int side;                           /* Side to move (WHITE or BLACK) */
    int enPas;                          /* En passant target square (or -1 if none) */
    int fiftyMove;                      /* Halfmove clock for the 50-move rule */
    int ply;                            /* Depth of current search in the tree */
    int hisPly;                         /* Total number of half-moves made in the game */
    int castlePerm;                     /* Castling permissions (bitmask or similar) */
    unsigned long long positionKey;     /* Zobrist hash key for the current position */
} Board;

/* Function prototypes */

/*
   InitBoard:
   - Resets the Board structure to a default (empty) state.
   - Sets side to WHITE, enPas to -1, castle permissions to 0, etc.
   - Typically called before loading a position or setting up a new game.
*/
void InitBoard(Board* b);

/*
   SetFen:
   - Sets the Board structure from a FEN string.
   - A typical FEN string includes piece placement, side to move, castling rights,
     en passant square, halfmove clock, and fullmove number.
   - This function first calls InitBoard, then parses the FEN to place pieces
     and set the associated fields.
   - If the FEN is invalid, the Board might remain partially set or default
     to the init state.
*/
void SetFen(Board* b, const char* fen);

/*
   PrintBoard:
   - Prints the board in ASCII format to stdout.
   - Shows ranks 8 down to 1, each file from 'a' to 'h'.
   - Also prints side to move, castling rights, en passant square,
     halfmove clock, ply, and position key.
   - Useful for debugging or console-based play.
*/
void PrintBoard(const Board* b);

#endif /* BOARD_H */
