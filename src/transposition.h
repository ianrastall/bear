/****************************************************************************
 * File: transposition.h
 ****************************************************************************/
/*
   Description:
   - Header for transposition table (TT) or hash table.
   - Declares the data structures and functions to store/retrieve board states.
   - This helps skip re-calculating positions that have already been analyzed.

   Functions to declare here:
   1) void InitTranspositionTable(TransTable* tt, size_t size);
   2) void StoreHashEntry(TransTable* tt, uint64_t key, ... other info ...);
   3) bool ProbeHashEntry(TransTable* tt, uint64_t key, ... out info ...);
   4) etc.

   Data structures:
   - TransTable struct, containing entries or pointers to them.
   - TTEntry struct with fields: key (Zobrist), depth, score, etc.
*/

#ifndef TRANSPOSITION_H
#define TRANSPOSITION_H

#include <stdint.h>  /* For uint64_t */
#include <stddef.h>  /* For size_t */
#include <stdbool.h> /* For bool */
#include "board.h"   /* For Board, if needed */

/* 
   Structure for a single transposition table entry.
   You can adjust or expand the fields for storing more info:
   - key: Zobrist hash key
   - depth: search depth at which this entry was computed
   - score: evaluation score or alpha/beta boundary
   - flag: indicates node type (e.g., PV, CUT, ALL)
   - move: best move found (if any)
   - age: used to track whether an entry is from an older search
*/
typedef struct {
    uint64_t key;
    int depth;
    int score;
    int flag;      /* 0 = exact, 1 = alpha, 2 = beta (or any scheme you like) */
    Move bestMove; /* optional: store best move for this position */
    int age;       /* helps replace old entries */
} TTEntry;

/* 
   Structure for the entire transposition table.
   - entries: pointer to allocated array of TTEntry
   - numEntries: total number of entries
   - newWrite: count of newly written or overwritten entries
   - age: increments every search, so we can track entry staleness
*/
typedef struct {
    TTEntry* entries;
    size_t numEntries;
    int newWrite;
    int age;
} TransTable;

/* Function prototypes */
void InitTranspositionTable(TransTable* tt, size_t size);
void FreeTranspositionTable(TransTable* tt);

void StoreHashEntry(TransTable* tt, uint64_t key, int depth, int score, 
                    int flag, Move bestMove);

bool ProbeHashEntry(TransTable* tt, uint64_t key, int depth, int* outScore,
                    Move* outMove);

#endif /* TRANSPOSITION_H */
