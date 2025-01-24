/****************************************************************************
 * File: transposition.h
 ****************************************************************************/
/*
   Description:
   - Header for transposition table (TT) or hash table.
   - Declares the data structures and functions to store/retrieve board states,
     typically via Zobrist hash keys to identify positions.
   - Helps speed up search by caching results of previously analyzed positions.

   Functions declared:
   1) void InitTranspositionTable(TransTable* tt, size_t size);
   2) void FreeTranspositionTable(TransTable* tt);
   3) void StoreHashEntry(TransTable* tt, uint64_t key, int depth, int score,
                          int flag, Move bestMove);
   4) bool ProbeHashEntry(TransTable* tt, uint64_t key, int depth, int* outScore,
                          Move* outMove);

   Data structures:
   - TTEntry: holds Zobrist key, search depth, score, node type flag, best move, etc.
   - TransTable: an array of TTEntry plus metadata.
*/

#ifndef TRANSPOSITION_H
#define TRANSPOSITION_H

#include <stdint.h>  /* For uint64_t */
#include <stddef.h>  /* For size_t */
#include <stdbool.h> /* For bool */
#include "movegen.h" /* For the Move struct */

/* 
   Structure for a single transposition table entry.
   Fields:
    - key:     Zobrist hash key
    - depth:   depth at which this entry was calculated
    - score:   stored evaluation or alpha/beta window boundary
    - flag:    node type (0=exact, 1=alpha, 2=beta, etc.)
    - bestMove:the best move found in this position
    - age:     to help with replacing old entries
*/
typedef struct {
    uint64_t key;
    int depth;
    int score;
    int flag;      /* e.g., 0=Exact, 1=Alpha, 2=Beta */
    Move bestMove; /* optional best move in this position */
    int age;       /* helps detect older entries to replace */
} TTEntry;

/*
   Structure for the entire transposition table:
    - entries:   dynamically allocated array of TTEntry
    - numEntries: number of entries allocated
    - newWrite:   count how many times we've stored new data
    - age:        increments each search iteration
*/
typedef struct {
    TTEntry* entries;
    size_t numEntries;
    int newWrite;
    int age;
} TransTable;

/* Allocate and initialize the TT. size = desired number of entries. */
void InitTranspositionTable(TransTable* tt, size_t size);

/* Free the memory associated with the TT and reset fields. */
void FreeTranspositionTable(TransTable* tt);

/* Store a new entry (or replace an older one) in the TT. */
void StoreHashEntry(TransTable* tt, uint64_t key, int depth, int score, 
                    int flag, Move bestMove);

/* Probe the TT for a matching entry with a key & sufficient depth. */
bool ProbeHashEntry(TransTable* tt, uint64_t key, int depth, int* outScore, 
                    Move* outMove);

#endif /* TRANSPOSITION_H */
