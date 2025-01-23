/****************************************************************************
 * File: defs.h
 ****************************************************************************/
/*
   Description:
   - General definitions and constants used throughout the engine.
   - Include macros for board size, special piece values, move encoding, etc.
   - Possibly define common data types or enumerations (piece types, colors).
   - We also provide some basic includes and a simple guard.

   This file serves as a central location for important global constants
   and macros that the rest of the engine can depend on.
*/

#ifndef DEFS_H
#define DEFS_H

/* Standard library includes that might be used in many files */
#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>  /* For bool, true, false in C99+ */
#include <string.h>   /* Often used throughout the engine */

/*
   Board dimensions / representation
   -------------------------------------------------
   We use the mailbox approach with a 120-square array:
     - Indices [0..119] are valid in the array.
     - The playable squares correspond to [21..98], skipping edges.
     - Each row (rank) is 10 squares wide, but only the middle 8 squares
       are "on board."

   Example layout:
   Row 0:  [ 0][ 1][ 2][ 3][ 4][ 5][ 6][ 7][ 8][ 9]
   Row 1:  [10][11][12][13][14][15][16][17][18][19]
   Row 2:  [20][21][22][23][24][25][26][27][28][29]
      ...
   Row 9:  [90][91][92][93][94][95][96][97][98][99]
   Row10:  [100][101][102][103][104][105][106][107][108][109]
   Row11:  [110][111][112][113][114][115][116][117][118][119]

   The actual 8x8 board for chess is embedded in rows 2..9, columns 1..8.
   Hence, each rank is offset by +2, and each file by +1.

   We'll define BOARD_SIZE to be 120, which covers all squares including the
   "off-board" edges.
*/
#define BOARD_SIZE 120

/*
   Search-related constants
   -------------------------------------------------
   MAX_DEPTH: Maximum depth we allow the search to go to.
   MAX_MOVES: An upper bound on how many moves can appear in a single position.
*/
#define MAX_DEPTH 64
#define MAX_MOVES 256

/*
   Colors
   -------------------------------------------------
   We define color constants for white, black, and sometimes both (used in
   certain hashing or housekeeping tasks).
*/
#define WHITE 0
#define BLACK 1
#define BOTH  2

/*
   Enumerations for piece types
   -------------------------------------------------
   We encode each piece as a unique integer so that we can store it in
   Board.pieces[] quickly.  The typical approach is:

   0 = EMPTY
   1 = W_PAWN
   2 = W_KNIGHT
   3 = W_BISHOP
   4 = W_ROOK
   5 = W_QUEEN
   6 = W_KING
   7 = B_PAWN
   8 = B_KNIGHT
   9 = B_BISHOP
   10 = B_ROOK
   11 = B_QUEEN
   12 = B_KING

   We do it this way so that we can quickly check if a piece is white
   or black, or if it is empty, etc.
*/
enum {
    EMPTY,
    W_PAWN, W_KNIGHT, W_BISHOP, W_ROOK, W_QUEEN, W_KING,
    B_PAWN, B_KNIGHT, B_BISHOP, B_ROOK, B_QUEEN, B_KING
};

/*
   Piece Values
   -------------------------------------------------
   We assign classical material values to the different pieces.
   These are used in the evaluation function and other heuristics.
   The exact values can vary by engine; these are typical ballpark.
*/
#define VAL_PAWN   100
#define VAL_KNIGHT 320
#define VAL_BISHOP 330
#define VAL_ROOK   500
#define VAL_QUEEN  900
#define VAL_KING   20000  /* Arbitrary large number to ensure losing the king is critical */

/*
   Useful Macros
   -------------------------------------------------
   Below are a few examples of macros commonly used in a chess engine.
   - Move Encoding Macros: If you choose to encode your moves in a single
     integer, you might have macros that manipulate bits for 'from', 'to',
     'captured', 'promoted', etc.
   - Hashing Constants: You might define constants for the size of your
     transposition table or random seeds for Zobrist hashing.
   - Indexing Macros: Converting from rank/file to the mailbox index.
*/

/* Example macros to convert (rank, file) to 120-based index.
   Here, rank and file are expected to be in [0..7]. */
#define FR2SQ(file, rank) ((rank + 2) * 10 + (file + 1))

/* 
   If you want to get the rank and file from a 120-based index, 
   you can do something like this:
   #define SQ2RANK(sq) ((sq) / 10 - 2)
   #define SQ2FILE(sq) ((sq) % 10 - 1)
*/

/* 
   We might want other macros to check if a 120-based index is on-board.
   For example:
   #define IS_SQ_ON_BOARD(sq) (file>=0 && file<8 && rank>=0 && rank<8)
   but usually we just check if pieces[sq] != OFFBOARD in a typical engine.
*/

/*
   Misc. engine-wide constants
   -------------------------------------------------
   You might choose to define macros for special move flags (e.g. en passant,
   castling, promotion). Some engines do this differently depending on their
   internal representation of moves. For example:
   #define MFLAG_EN_PASSANT (1 << 0)
   #define MFLAG_PAWN_START (1 << 1)
   #define MFLAG_CASTLING   (1 << 2)
   #define MFLAG_PROMOTION  (1 << 3)
   etc.
*/

/*
   End of DEFS_H guard
*/
#endif /* DEFS_H */
