/****************************************************************************
 * File: evaluate.h
 ****************************************************************************/
/*
   Description:
   - Header for the evaluation module.
   - Declares a function for evaluating the board from one side's perspective.
   - If not using NNUE, it might just be classical heuristics (material count,
     piece-square tables, mobility, etc.).
   - This file is included by other engine modules that need to evaluate
     the current position, such as the search or quiescence functions.

   Data structures / Additional definitions:
   - You might define piece-square tables or other heuristics here if you
     need them accessible from multiple files. Alternatively, keep them
     local to evaluate.c if only used there.
   - You could define constants for special bonuses or penalties, like
     #define BISHOP_PAIR_BONUS 30, if they need to be shared or tweaked
     globally.
*/

#ifndef EVALUATE_H
#define EVALUATE_H

#include "board.h"

/*
   EvaluatePosition:
   -----------------------------------------------------------------------
   - Evaluates the current board position and returns an integer score:
       > 0 means the position is better for White,
       < 0 means better for Black,
         0 means (roughly) equal.

   - The function typically accounts for:
       1) Material balance (pawn = 100, knight/bishop ~ 300, rook ~ 500, queen ~ 900).
       2) Piece-Square Tables (PST) to account for piece placement.
       3) Minor heuristics such as:
           - Bishop pair bonus
           - Rooks on open or semi-open files
           - King safety considerations
       4) Any other relevant positional or strategic factors.

   - Implementation details:
       - The specifics of how you combine these heuristics can be tuned
         to your engine's style and tested for performance strength.
       - Some engines keep a "white-based" evaluation (positive = good for White),
         while others might invert the score depending on who is to move.

   Parameters:
   - const Board* b: A pointer to the current board structure, containing
                     piece placement, side to move, castling rights, etc.

   Returns:
   - An integer score, with positive favoring White and negative favoring Black.
*/
int EvaluatePosition(const Board* b);

#endif /* EVALUATE_H */
