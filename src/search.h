/****************************************************************************
 * File: search.h
 ****************************************************************************/
/*
    Description:
    - Header for the search routines (alpha-beta, quiescence, etc.).
    - Declares the SearchInfo struct to track search parameters and results.
*/

#ifndef SEARCH_H
#define SEARCH_H

#include "board.h"
#include "movegen.h"
#include "evaluate.h"
#include "transposition.h"

/* Structure to hold search parameters and results */
typedef struct {
    int depth;          /* Maximum search depth */
    int movetime;       /* Time allocated for the move in milliseconds */
    int startTime;      /* Search start time (e.g., ms) */
    int stopTime;       /* When we must stop searching */
    long nodes;         /* Number of nodes visited in this search */
    bool timeSet;       /* True if a time limit is set */
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
