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
*/

#include "uci.h"

#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

/*
    UciLoop:
    - Continuously reads lines from stdin.
    - Breaks on "quit" command or EOF.
    - Calls ParseUciCommand for each line.
*/
void UciLoop(Board* board, TransTable* tt)
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
        ParseUciCommand(line, board, tt);
    }
}

/*
    ParseUciCommand:
    - Identifies UCI commands from the input line and
      performs corresponding actions.
    - Handles commands: "uci", "isready", "position", "go", "stop", "ucinewgame".
*/
void ParseUciCommand(const char* line, Board* board, TransTable* tt)
{
    /* "uci" command: 
       - Engine must respond with "id name ...", "id author ...", then "uciok". */
    if (!strcmp(line, "uci")) {
        printf("id name Bear 0.01\n");
        printf("id author ChatGPT o1\n");
        printf("uciok\n");
        LogMessage(LOG_INFO, "Handled 'uci' command.\n");
    }
    /* "isready" command:
       - Engine must respond "readyok" when fully ready. */
    else if (!strcmp(line, "isready")) {
        /* Perform any necessary preparations here */
        printf("readyok\n");
        LogMessage(LOG_INFO, "Handled 'isready' command.\n");
    }
    /* "position" command:
       - Example: "position startpos moves e2e4 e7e5"
       - or: "position fen <FEN> moves ..."
       - Parses the position and updates the board accordingly.
    */
    else if (!strncmp(line, "position", 8)) {
        LogMessage(LOG_DEBUG, "Handling 'position' command: %s\n", line);
        
        /* Tokenize the input */
        char copy[1024];
        strncpy(copy, line, sizeof(copy));
        copy[sizeof(copy)-1] = '\0'; /* Ensure null-termination */

        char* token = strtok(copy, " ");
        token = strtok(NULL, " "); /* Next token after "position" */

        if (token && !strcmp(token, "startpos")) {
            InitBoard(board);
            LogMessage(LOG_DEBUG, "Initialized board to start position.\n");
            token = strtok(NULL, " "); /* Check for "moves" */
        }
        else if (token && !strcmp(token, "fen")) {
            char fen[256] = {0};
            token = strtok(NULL, " "); /* Start of FEN string */
            if (token) {
                /* Reconstruct the full FEN string */
                while (token && strlen(fen) < sizeof(fen) - 1) {
                    strcat(fen, token);
                    strcat(fen, " ");
                    token = strtok(NULL, " ");
                }
                /* Remove the trailing space */
                size_t len = strlen(fen);
                if (len > 0 && fen[len-1] == ' ') {
                    fen[len-1] = '\0';
                }
                SetFen(board, fen, true); /* Assuming debugMode is enabled */
                LogMessage(LOG_DEBUG, "Set board from FEN: %s\n", fen);
            }
        }

        /* Handle "moves" if present */
        token = strtok(NULL, " ");
        if (token && !strcmp(token, "moves")) {
            LogMessage(LOG_DEBUG, "Applying moves from 'position' command.\n");
            while ((token = strtok(NULL, " ")) != NULL) {
                /* Convert UCI move string to Move struct */
                Move move = UciMoveToMove(board, token);
                if (IsMoveLegal(board, move)) {
                    MakeMove(board, move);
                    LogMessage(LOG_DEBUG, "Applied move: %s\n", token);
                }
                else {
                    LogMessage(LOG_WARN, "Illegal move attempted: %s\n", token);
                }
            }
        }
    }
    /* "go" command:
       - Tells engine to start searching.
       - Parses optional parameters like time controls or depth.
    */
    else if (!strncmp(line, "go", 2)) {
        LogMessage(LOG_DEBUG, "Handling 'go' command: %s\n", line);
        
        /* Initialize SearchInfo */
        SearchInfo info;
        ClearSearchInfo(&info);
        
        /* Parse the "go" command parameters */
        char copy[1024];
        strncpy(copy, line, sizeof(copy));
        copy[sizeof(copy)-1] = '\0'; /* Ensure null-termination */

        char* token = strtok(copy, " ");
        token = strtok(NULL, " "); /* Next token after "go" */

        while (token) {
            if (!strcmp(token, "depth")) {
                token = strtok(NULL, " ");
                if (token) {
                    info.depth = atoi(token);
                    LogMessage(LOG_DEBUG, "Search depth set to %d.\n", info.depth);
                }
            }
            else if (!strcmp(token, "movetime")) {
                token = strtok(NULL, " ");
                if (token) {
                    info.movetime = atoi(token); /* Assuming SearchInfo has movetime */
                    LogMessage(LOG_DEBUG, "Search movetime set to %d ms.\n", info.movetime);
                }
            }
            // Handle other parameters like wtime, btime, etc.
            // ...
            token = strtok(NULL, " ");
        }

        /* Start the search (this is a blocking call) */
        int score = SearchPosition(board, &info);
        LogMessage(LOG_DEBUG, "Search completed with score: %d.\n", score);

        /* Output the best move */
        char bestMoveStr[10];
        MoveToUciMove(info.bestMove, bestMoveStr); /* Convert Move struct to UCI string */
        printf("bestmove %s\n", bestMoveStr);
        LogMessage(LOG_INFO, "Best move: %s\n", bestMoveStr);
    }
    /* "stop" command:
       - Tells engine to stop searching immediately and output the best move found. */
    else if (!strcmp(line, "stop")) {
        LogMessage(LOG_DEBUG, "Handling 'stop' command.\n");
        /* Implement search stopping mechanism here */
        /* Example: set info.stopped = true; */
        /* Then, after search detects the stop, it should output the best move */
    }
    /* "ucinewgame" command:
       - Signals a new game is about to start. Typically we reset the board,
         transposition table, search stats, etc. */
    else if (!strcmp(line, "ucinewgame")) {
        LogMessage(LOG_INFO, "Handling 'ucinewgame' command.\n");
        /* Reset the board */
        InitBoard(board);
        /* Optionally clear the transposition table */
        /* FreeTranspositionTable(tt);
           InitTranspositionTable(tt, tt->numEntries); */
        /* Reset search statistics if any */
    }
    /* Otherwise, it's an unknown or unhandled command. */
    else {
        LogMessage(LOG_WARN, "Received unknown command: %s\n", line);
        /* Optionally, you can respond with something or ignore */
    }
}

/*
    Helper Functions (Implement these based on your engine's design)
*/

/*
    UciMoveToMove:
    - Converts a UCI move string (e.g., "e2e4") to a Move struct.
    - Handles promotions (e.g., "e7e8q").
*/
Move UciMoveToMove(Board* board, const char* uci)
{
    Move move = {0};
    /* Example implementation:
       - Convert file/rank to square indices
       - Handle promotions
       - Set flags if necessary (e.g., en passant, castling)
    */

    /* Basic parsing for from and to squares */
    if (strlen(uci) < 4) {
        LogMessage(LOG_ERROR, "Invalid UCI move format: %s\n", uci);
        return move;
    }

    int fromFile = uci[0] - 'a';
    int fromRank = uci[1] - '1';
    int toFile   = uci[2] - 'a';
    int toRank   = uci[3] - '1';

    move.from = FRTo120(fromFile, fromRank);
    move.to   = FRTo120(toFile, toRank);

    /* Handle promotion */
    if (strlen(uci) == 5) {
        char promo = tolower(uci[4]);
        switch(promo) {
            case 'q': move.promoted = (board->side == WHITE) ? W_QUEEN : B_QUEEN; break;
            case 'r': move.promoted = (board->side == WHITE) ? W_ROOK  : B_ROOK;  break;
            case 'b': move.promoted = (board->side == WHITE) ? W_BISHOP: B_BISHOP; break;
            case 'n': move.promoted = (board->side == WHITE) ? W_KNIGHT: B_KNIGHT; break;
            default:
                move.promoted = EMPTY;
                LogMessage(LOG_WARN, "Unknown promotion piece: %c\n", promo);
                break;
        }
        move.flag |= (1 << 3); /* Promotion flag */
    }

    /* Add additional logic for en passant, castling, etc. */

    return move;
}

/*
    MoveToUciMove:
    - Converts a Move struct to a UCI move string (e.g., "e2e4").
    - Handles promotions (e.g., "e7e8q").
*/
void MoveToUciMove(Move move, char* uci)
{
    /* Example implementation:
       - Convert square indices back to file/rank
       - Append promotion character if necessary
    */

    /* Basic conversion for from and to squares */
    int fromFile = (move.from % 10) - 1;
    int fromRank = (move.from / 10) - 2;
    int toFile   = (move.to % 10) - 1;
    int toRank   = (move.to / 10) - 2;

    sprintf(uci, "%c%d%c%d", 'a' + fromFile, 1 + fromRank, 'a' + toFile, 1 + toRank);

    /* Handle promotion */
    if (move.flag & (1 << 3)) { /* Promotion flag */
        char promo = 'q'; /* Default to queen */
        switch(move.promoted) {
            case W_ROOK:
            case B_ROOK:
                promo = 'r';
                break;
            case W_BISHOP:
            case B_BISHOP:
                promo = 'b';
                break;
            case W_KNIGHT:
            case B_KNIGHT:
                promo = 'n';
                break;
            case W_QUEEN:
            case B_QUEEN:
            default:
                promo = 'q';
                break;
        }
        sprintf(uci + strlen(uci), "%c", promo);
    }

    /* Add additional logic for en passant, castling, etc. */
}
