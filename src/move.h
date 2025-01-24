/****************************************************************************
 * File: move.h
 ****************************************************************************/
/*
    Description:
    - Header defining the Move structure used across the engine.
    - Ensures that any file needing to use Move can include this header
      without introducing circular dependencies.
*/

#ifndef MOVE_H
#define MOVE_H

#include <stdbool.h>

/* Define the Move structure */
typedef struct {
    int from;       /* The source square (120-based indexing) */
    int to;         /* The target square (120-based indexing) */
    int captured;   /* Piece type captured, if any (EMPTY if none) */
    int promoted;   /* Piece type if this move is a promotion (EMPTY if none) */
    int flag;       /* Bitmask for special move attributes (e.g., en passant, castling, promotion) */
} Move;

/* Forward declaration of Board struct */
struct Board;

/* Function prototypes related to Move conversions */
Move UciMoveToMove(struct Board* board, const char* uci); /* Converts UCI move string to Move struct */
void MoveToUciMove(Move move, char* uci);          /* Converts Move struct to UCI move string */

#endif /* MOVE_H */
