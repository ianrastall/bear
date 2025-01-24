/****************************************************************************
 * File: main.c
 ****************************************************************************/
/*
   Main entry point for the Bear chess engine.
   Handles initialization, command-line arguments, and enters the UCI loop.
*/

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "defs.h"
#include "board.h"
#include "movegen.h"
#include "search.h"
#include "evaluate.h"
#include "transposition.h"
#include "uci.h"
#include "log.h" /* Include logging header */

int main(int argc, char* argv[])
{
    printf("Engine name: Bear 0.01\n");
    printf("Author: ChatGPT o1\n\n");

    int debugMode = 0; // Initialize debugMode
    size_t ttSize = 1024 * 1024; /* Example: number of TT entries */

    /* Process command-line arguments */
    for(int i = 1; i < argc; ++i) {
        if(!strcmp(argv[i], "--debug")) {
            debugMode = 1;
            printf("Debug mode: ON\n");
        }
        else if(!strcmp(argv[i], "--tt-size") && i + 1 < argc) {
            ttSize = (size_t)atoi(argv[++i]);
            printf("Requested TT size: %zu entries\n", ttSize);
        }
    }

    /* Initialize logging */
    InitLogging(debugMode);
    SetLogLevel(LOG_DEBUG); /* Set desired log level */

    /* Initialize engine components */
    Board board;
    InitBoard(&board);
    LogMessage(LOG_DEBUG, "Board initialized.\n");

    TransTable tt;
    InitTranspositionTable(&tt, ttSize);
    LogMessage(LOG_DEBUG, "Transposition Table initialized with %zu entries.\n", ttSize);

    /* Optionally set up the board with a FEN or start position */
    // SetFen(&board, "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", debugMode);

    /* Enter UCI loop */
    LogMessage(LOG_DEBUG, "Entering UCI loop...\n");
    UciLoop(&board, &tt);
    LogMessage(LOG_DEBUG, "Exited UCI loop.\n");

    /* Clean up before exit */
    FreeTranspositionTable(&tt);
    LogMessage(LOG_DEBUG, "Transposition Table freed.\n");

    printf("Engine exiting.\n");
    return 0;
}
