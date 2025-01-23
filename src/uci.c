/****************************************************************************
 * File: uci.c
 ****************************************************************************/
/*
   Description:
   - Implementation of the UCI interface.
   - The main loop reads lines from stdin, parses them, and calls engine
     functions accordingly.
   - Must handle "uci", "isready", "position", "go", "stop", "quit" commands,
     plus any custom commands.

   Implementation details to fill later:
   1) Handling each command separately.
   2) Printing UCI-compliant responses (id name, id author, uciok, etc.).
   3) Sending "bestmove" at the end of a search.
*/

#include "uci.h"

/*
   UciLoop:
   - Reads lines from stdin in a loop.
   - Calls ParseUciCommand(line) to handle each command.
   - Usually exits when "quit" is received.
*/
void UciLoop(void)
{
    char line[1024];

    while (1) {
        /* Clear line buffer before reading */
        memset(line, 0, sizeof(line));

        if (!fgets(line, sizeof(line), stdin)) {
            /* If there's an input error or EOF, break */
            break;
        }

        /* Remove trailing newline, if any */
        char* nl = strchr(line, '\n');
        if (nl) *nl = '\0';

        if (!strcmp(line, "quit")) {
            break; /* Exit the UCI loop */
        }

        ParseUciCommand(line);
    }
}

/*
   ParseUciCommand:
   - Interprets a single line of command input and performs actions accordingly.
   - Typical UCI commands:
     "uci", "isready", "position", "go", "stop", "ucinewgame", etc.
*/
void ParseUciCommand(const char* line)
{
    if (!strcmp(line, "uci")) {
        /* 
           Print engine info, in the format:
           id name <engineName>
           id author <engineAuthor>
           uciok
        */
        printf("id name Bear 0.01\n");
        printf("id author ChatGPT o1\n");
        printf("uciok\n");
    }
    else if (!strcmp(line, "isready")) {
        /* Engine should respond with "readyok" when ready */
        printf("readyok\n");
    }
    else if (!strncmp(line, "position", 8)) {
        /* 
           "position startpos" or "position fen <FEN> moves ..."
           You would parse the FEN or start position,
           then make any subsequent moves.
        */
        // Example: position startpos moves e2e4 e7e5
        // (Placeholder for actual logic)
    }
    else if (!strncmp(line, "go", 2)) {
        /* 
           Start the search. Could parse time controls or depth, e.g.,
           go wtime 300000 btime 300000 winc 2000 binc 2000 movestogo ...
           Then call SearchPosition with the appropriate parameters.
        */
        // (Placeholder for actual logic)
    }
    else if (!strcmp(line, "stop")) {
        /* Stop searching and output bestmove. */
        // (Placeholder for actual logic, e.g., stop search thread if multithreaded)
    }
    else if (!strcmp(line, "ucinewgame")) {
        /* Reset the board, transposition table, etc. */
        // (Placeholder for actual logic)
    }
    else {
        /* Unknown command or not yet handled. */
    }
}
