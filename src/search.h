/****************************************************************************
 * File: search.h
 ****************************************************************************/
/*
   Description:
   - Header for the search routines (alpha-beta, quiescence, etc.).
   - Declares a SearchInfo struct to track search parameters and results.

   Exports:
     1) int SearchPosition(Board* b, SearchInfo* info);
     2) int AlphaBeta(Board* b, int alpha, int beta, int depth, SearchInfo* info);
     3) int Quiescence(Board* b, int alpha, int beta, SearchInfo* info);
     4) void ClearSearchInfo(SearchInfo* info);
*/

#ifndef SEARCH_H
#define SEARCH_H

#include "board.h"
#include "movegen.h"   /* Needed for Move struct, move generation */
#include "evaluate.h"  /* If your search calls EvaluatePosition() directly */

/* Holds data about the current search (time control, best move, etc.). */
typedef struct {
    int depth;          /* Maximum search depth */
    int startTime;      /* Search start time (e.g., ms) */
    int stopTime;       /* When we must stop searching */
    long nodes;         /* Number of nodes visited in this search */
    bool timeSet;       /* True if we have a time limit */
    bool stopped;       /* Set to true if we must stop immediately */
    Move bestMove;      /* Store the best move found */
    /* Additional fields can be added as needed */
} SearchInfo;

/* Function prototypes */
void ClearSearchInfo(SearchInfo* info);
int  SearchPosition(Board* b, SearchInfo* info);
int  AlphaBeta(Board* b, int alpha, int beta, int depth, SearchInfo* info);
int  Quiescence(Board* b, int alpha, int beta, SearchInfo* info);

#endif /* SEARCH_H */
