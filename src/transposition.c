/****************************************************************************
 * File: transposition.c
 ****************************************************************************/
/*
   Description:
   - Implementation of the transposition table (TT) functions.
   - Typically uses Zobrist hashing to generate a unique key for each board.
   - Stores depth, score, and best move data to speed up future searches.

   Implementation details:
   1) Memory allocation for TT entries.
   2) Simple replacement scheme for collisions (replace if depth >= current entry).
   3) Retrieving (probing) an entry if the key matches and depth is sufficient.
*/

#include "transposition.h"
#include <stdio.h>   /* For fprintf, stderr */
#include <stdlib.h>  /* For calloc, free */

/*
   InitTranspositionTable:
   - Allocates TTEntry array of given size.
   - Resets counters (newWrite, age).
*/
void InitTranspositionTable(TransTable* tt, size_t size)
{
    tt->entries = (TTEntry*)calloc(size, sizeof(TTEntry));
    if (!tt->entries) {
        fprintf(stderr, "Error: Unable to allocate memory for Transposition Table\n");
        tt->numEntries = 0;
        return;
    }
    tt->numEntries = size;
    tt->newWrite   = 0;
    tt->age        = 0;
}

/*
   FreeTranspositionTable:
   - Frees the allocated TT array (if any) and resets fields.
*/
void FreeTranspositionTable(TransTable* tt)
{
    if (tt->entries) {
        free(tt->entries);
        tt->entries = NULL;
    }
    tt->numEntries = 0;
    tt->newWrite   = 0;
    tt->age        = 0;
}

/*
   StoreHashEntry:
   - Basic hash function using key % numEntries as index.
   - If the slot is empty (key=0), or matches the same key, or new depth >= old depth,
     we overwrite. Otherwise, do nothing (simple approach).
   - In practice, you'd want a more robust collision scheme (e.g., 2-slot, buckets, etc.).
*/
void StoreHashEntry(TransTable* tt, uint64_t key, int depth, int score, 
                    int flag, Move bestMove)
{
    if (!tt->entries || tt->numEntries == 0) return;

    size_t index = key % tt->numEntries;
    TTEntry* entry = &tt->entries[index];

    /* Check if entry is empty or same key or we have deeper depth => replace */
    if (entry->key == 0ULL || entry->key == key || depth >= entry->depth) {
        entry->key      = key;
        entry->depth    = depth;
        entry->score    = score;
        entry->flag     = flag;
        entry->bestMove = bestMove;
        entry->age      = tt->age;
        tt->newWrite++;
    }
    /* else: keep the old entry (simple collision policy) */
}

/*
   ProbeHashEntry:
   - Check if there's an entry with the same key.
   - If found and entry->depth >= requested depth, we consider it valid.
   - Returns true if successful, false otherwise. outScore/outMove are set if found.
*/
bool ProbeHashEntry(TransTable* tt, uint64_t key, int depth, int* outScore, 
                    Move* outMove)
{
    if (!tt->entries || tt->numEntries == 0) {
        return false;
    }

    size_t index = key % tt->numEntries;
    TTEntry* entry = &tt->entries[index];

    /* Check for a match and if the stored depth is >= required depth. */
    if (entry->key == key && entry->depth >= depth) {
        if (outScore) {
            *outScore = entry->score;
        }
        if (outMove) {
            *outMove = entry->bestMove;
        }
        return true;
    }

    return false;
}
