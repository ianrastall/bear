/****************************************************************************
 * File: uci.c
 ****************************************************************************/
/*
   Description:
   - Implementation of the UCI interface for Bear chess engine.
   - The UciLoop reads lines from stdin and calls ParseUciCommand to handle
     each command. Exits on "quit".
   - ParseUciCommand dispatches commands like "uci", "isready", "position",
     "go", "stop", "ucinewgame", etc.

   Placeholder logic is provided; you can fill in details for position setup,
   search invocation, etc.
*/

#include "uci.h"

#include <stdbool.h>
#include <stdlib.h>

/*
   UciLoop:
   - Continuously reads lines from stdin.
   - Breaks on "quit" command or EOF.
   - Calls ParseUciCommand for each line.
*/
void UciLoop(void)
{
    char line[1024];

    while (true) {
        /* Clear the line buffer before reading */
        memset(line, 0, sizeof(line));

        /* If we fail to read (EOF or error), exit loop */
        if (!fgets(line, sizeof(line), stdin)) {
            break;
        }

        /* Remove trailing newline, if any */
        char* nl = strchr(line, '\n');
        if (nl) *nl = '\0';

        /* Check for "quit" explicitly */
        if (!strcmp(line, "quit")) {
            break;
        }

        /* Parse the command */
        ParseUciCommand(line);
    }
}

/*
   ParseUciCommand:
   - Identifies UCI commands from the input line and
     performs the corresponding actions.
   - Placeholder logic for advanced commands (position, go, etc.).
*/
void ParseUciCommand(const char* line)
{
    /* "uci" command: 
       - Engine must respond with "id name ...", "id author ...", then "uciok". */
    if (!strcmp(line, "uci")) {
        printf("id name Bear 0.01\n");
        printf("id author ChatGPT o1\n");
        printf("uciok\n");
    }
    /* "isready" command:
       - Engine must respond "readyok" when fully ready. */
    else if (!strcmp(line, "isready")) {
        // Possibly flush or wait for background threads to be ready
        printf("readyok\n");
    }
    /* "position" command:
       - Example: "position startpos moves e2e4 e7e5"
       - or: "position fen <FEN> moves ..."
       - Typically, we parse the FEN if present or use the standard start position,
         then apply the moves listed after "moves". */
    else if (!strncmp(line, "position", 8)) {
        // Example parsing logic:
        // "position startpos"
        // "position startpos moves e2e4 e7e5"
        // "position fen <fenstring> moves ..."
        // You would call InitBoard(&board) or SetFen(&board, "fenstring");
        // Then replay subsequent moves to reach the final position.
        // (Implementation placeholder)
    }
    /* "go" command:
       - Tells engine to start searching. Additional parameters might follow:
         "go wtime 300000 btime 300000 winc 2000 binc 2000 movestogo 30 depth 10" etc.
       - You parse those parameters, set up a SearchInfo, and call SearchPosition. */
    else if (!strncmp(line, "go", 2)) {
        // Example: parse time controls, depth, etc.
        // Then call SearchPosition(&board, &info).
        // (Implementation placeholder)
    }
    /* "stop" command:
       - Tells engine to stop searching immediately and output the best move found. */
    else if (!strcmp(line, "stop")) {
        // If the search is running in a separate thread or loop, you set a stop flag.
        // Then eventually print "bestmove <move>".
        // (Implementation placeholder)
    }
    /* "ucinewgame" command:
       - Signals a new game is about to start. Typically we reset the board,
         transposition table, search stats, etc. */
    else if (!strcmp(line, "ucinewgame")) {
        // Clear/Init board, TT, etc.
        // (Implementation placeholder)
    }
    /* Otherwise, it's an unknown or unhandled command. */
    else {
        // Could do nothing or print something about unrecognized command.
        // (No action needed if you want silent ignoring.)
    }
}
