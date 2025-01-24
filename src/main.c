/****************************************************************************
 * File: main.c
 ****************************************************************************/
/*
   This is the main entry point for the chess engine "Bear 0.01". It uses
   the UCI protocol for communication with GUIs. 
*/

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* 
   Includes for engine modules, as needed.
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
   - Prints version info.
   - Processes command-line arguments (optional).
   - Initializes board, transposition table, etc.
   - Enters the UCI loop for engine communication.
   - Cleans up and exits.
*/
int main(int argc, char* argv[])
{
    printf("Engine name: Bear 0.01\n");
    printf("Author: ChatGPT o1\n\n");

    int debugMode = 0;
    size_t ttSize = 1024 * 1024; /* Example: in MB or raw entries */

    /* Process command-line arguments (optional). */
    for(int i = 1; i < argc; ++i) {
        if(!strcmp(argv[i], "--debug")) {
            debugMode = 1;
            printf("Debug mode: ON\n");
        }
        else if(!strcmp(argv[i], "--tt-size") && i + 1 < argc) {
            ttSize = (size_t)atoi(argv[++i]) * 1024 * 1024;
            printf("Requested TT size: %zu entries\n", ttSize);
        }
    }

    /* Initialize engine components */
    Board board;
    InitBoard(&board);

    TransTable tt;
    InitTranspositionTable(&tt, ttSize);

    /* Optionally do more initialization here (e.g., hashing seeds). */

    /* Enter UCI loop */
    UciLoop();

    /* Clean up before exit */
    FreeTranspositionTable(&tt);

    return 0;
}
