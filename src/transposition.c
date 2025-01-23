/****************************************************************************
 * File: transposition.c
 ****************************************************************************/
/*
   Description:
   - Implementation of the transposition table.
   - Usually uses Zobrist hashing to generate a unique key for each board.
   - Can store best move, node type (PV, CUT, ALL), depth, and evaluation score.
   - Helps with search speedups.

   Implementation details to fill later:
   1) Allocating and clearing the memory for the hash table.
   2) Handling collisions by replace, cluster, or 2-tier system.
   3) Reading/writing TT entries, including age or generation counters.
*/

#include "transposition.h"
#include <stdio.h>
#include <stdlib.h>

/*
   InitTranspositionTable:
   - Allocates memory for TT entries.
   - 'size' might be the desired number of entries (power of two is common).
   - Sets fields to defaults.
*/
void InitTranspositionTable(TransTable* tt, size_t size)
{
    tt->entries = (TTEntry*)calloc(size, sizeof(TTEntry));
    if (!tt->entries) {
        fprintf(stderr, "Error: Unable to allocate memory for TT\n");
        tt->numEntries = 0;
        return;
    }
    tt->numEntries = size;
    tt->newWrite = 0;
    tt->age = 0;
}

/*
   FreeTranspositionTable:
   - Frees the allocated TT memory to prevent leaks.
*/
void FreeTranspositionTable(TransTable* tt)
{
    if (tt->entries) {
        free(tt->entries);
        tt->entries = NULL;
    }
    tt->numEntries = 0;
    tt->newWrite = 0;
    tt->age = 0;
}

/*
   StoreHashEntry:
   - Finds a slot in the transposition table for the given key.
   - If there's a collision, decides whether to replace the existing entry
     based on depth, age, or some replacement scheme.
   - Writes data (depth, score, flag, bestMove) to the TTEntry.
*/
void StoreHashEntry(TransTable* tt, uint64_t key, int depth, int score, 
                    int flag, Move bestMove)
{
    if (!tt->entries || tt->numEntries == 0) return;

    /* Basic approach: use key mod numEntries as index (very simple). 
       In a real engine, you'd likely do more sophisticated collision handling.
    */
    size_t index = key % tt->numEntries;
    TTEntry* entry = &tt->entries[index];

    /* If empty or same key, or new depth >= existing depth, we replace. */
    if (entry->key == 0ULL || entry->key == key || depth >= entry->depth) {
        entry->key = key;
        entry->depth = depth;
        entry->score = score;
        entry->flag = flag;
        entry->bestMove = bestMove;
        entry->age = tt->age;
        tt->newWrite++;
    }
    else {
        /* If we want to do a "replace always" or "replace if older" scheme,
           we can handle that here. For now, do nothing if there's a conflict. */
    }
}

/*
   ProbeHashEntry:
   - Looks up the entry for the given key.
   - If found and the stored depth is sufficient, returns true and 
     outputs the stored score and move. Otherwise returns false.
*/
bool ProbeHashEntry(TransTable* tt, uint64_t key, int depth, int* outScore, 
                    Move* outMove)
{
    if (!tt->entries || tt->numEntries == 0) return false;

    size_t index = key % tt->numEntries;
    TTEntry* entry = &tt->entries[index];

    if (entry->key == key && entry->depth >= depth) {
        /* Found a valid entry with sufficient depth */
        *outScore = entry->score;
        if (outMove) {
            *outMove = entry->bestMove;
        }
        return true;
    }

    return false;
}
