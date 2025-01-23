/****************************************************************************
 * File: main.c
 ****************************************************************************/
/*
   This is the main entry point for the chess engine, which we are calling
   "Bear 0.01". It uses the UCI protocol for communication with GUIs. This
   file prints out engine information on startup, initializes necessary
   components, and enters the UCI loop for command processing.

   Possible enhancements in this file include:
   - Handling command-line arguments for debug mode or custom settings.
   - Initializing all global or static data structures.
   - Setting up a logging mechanism if desired.
   - Possibly reading config files or parameter files, etc.
*/

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* 
   Include any header files that define the engine's structures and
   initialization functions.
*/
#include "defs.h"
#include "board.h"
#include "movegen.h"
#include "search.h"
#include "evaluate.h"
#include "transposition.h"
#include "uci.h"

/*
   main:
   -------------------------------------------------------------------------
   - Prints version and author info to stdout.
   - Optionally processes command-line arguments.
   - Initializes key engine components:
       1) Board
       2) Transposition table (if using one)
       3) Any global data or logging
   - Calls UciLoop() for the main UCI protocol loop.
   - Cleans up resources before exiting.
*/
int main(int argc, char* argv[])
{
    /* Print version info on startup */
    printf("Engine name: Bear 0.01\n");
    printf("Author: ChatGPT o1\n\n");

    /* 
       1) Process command-line arguments (optional).
          For example, we could accept:
            --debug   : enable debugging
            --tt-size : specify transposition table size in MB
       This is just an example; we won't implement fully here.
    */
    int debugMode = 0;
    size_t ttSize = 1024 * 1024; /* In entries or bytes, depending on usage */

    for(int i = 1; i < argc; ++i) {
        if(!strcmp(argv[i], "--debug")) {
            debugMode = 1;
            printf("Debug mode: ON\n");
        }
        else if(!strcmp(argv[i], "--tt-size") && (i + 1 < argc)) {
            ttSize = (size_t)atoi(argv[++i]) * 1024 * 1024;
            printf("Requested TT size: %zu entries\n", ttSize);
        }
    }

    /* 
       2) Initialize engine components.
          - Board
          - Transposition table
          - Possibly other global data (like random seeds for Zobrist, if needed)
    */
    Board board;
    InitBoard(&board);

    TransTable tt;
    InitTranspositionTable(&tt, ttSize);

    /*
       If you have other global structures for evaluation or logging, 
       you might initialize them here as well.
    */

    /* 
       3) Enter the UCI loop:
          - This reads commands from stdin (like "uci", "isready", "position", "go").
          - Implementation details are in uci.c, specifically UciLoop().
    */
    UciLoop();

    /*
       4) Clean up before exit:
          - Free dynamic memory allocations (like the TT).
          - Save logs or stats if needed.
    */
    FreeTranspositionTable(&tt);

    return 0;
}
