/****************************************************************************
 * File: search.h
 ****************************************************************************/
/*
   Description:
   - Header for the search routines.
   - Declarations for alpha-beta (or whichever search method), iterative deepening,
     aspiration windows, quiescence search, etc.
   - Possibly define a 'SearchInfo' struct for depth, time management, etc.

   Functions to declare here:
   1) int SearchPosition(Board* b, SearchInfo* info);
   2) int AlphaBeta(Board* b, int alpha, int beta, int depth, SearchInfo* info);
   3) int Quiescence(Board* b, int alpha, int beta, SearchInfo* info);
   4) void ClearSearchInfo(SearchInfo* info);
   5) etc.

   Data structures:
   - A 'SearchInfo' struct to hold best move, search depth, allocated time,
     nodes, etc.
*/

#ifndef SEARCH_H
#define SEARCH_H

#include "board.h"
#include "movegen.h"  /* Potentially needed for moves */
#include "evaluate.h" /* For evaluating positions */

/* 
   SearchInfo struct:
   - Holds data about the current search, such as the best move,
     search depth, allocated time, number of nodes visited, etc.
   - You can adjust/expand as needed.
*/
typedef struct {
    int depth;          /* Max search depth */
    int startTime;      /* When the search started (could be a timestamp) */
    int stopTime;       /* When we need to stop the search (for time control) */
    long nodes;         /* Count of positions (nodes) searched */
    bool timeSet;       /* Indicates whether we have a time limit */
    bool stopped;       /* Indicates if we should stop the search */
    Move bestMove;      /* Store the best move found */
    /* More fields can be added for aspiration windows, etc. */
} SearchInfo;

/* Function prototypes */
int SearchPosition(Board* b, SearchInfo* info);
int AlphaBeta(Board* b, int alpha, int beta, int depth, SearchInfo* info);
int Quiescence(Board* b, int alpha, int beta, SearchInfo* info);
void ClearSearchInfo(SearchInfo* info);

#endif /* SEARCH_H */
