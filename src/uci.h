/****************************************************************************
 * File: uci.h
 ****************************************************************************/
/*
    Description:
    - Header for handling UCI (Universal Chess Interface) commands and loop.
    - Declares functions to read commands from stdin, parse them, and
      execute corresponding engine actions (like setting positions, starting
      a search, etc.).
*/

#ifndef UCI_H
#define UCI_H

#include <stdio.h>
#include <string.h>
#include "board.h"
#include "search.h"
#include "movegen.h"
#include "transposition.h"
#include "evaluate.h"
#include "log.h" /* For logging */
#include "move.h" /* Ensure Move is defined */

/*
    UciLoop:
    - Reads lines from stdin and handles them until "quit" is received.
    - Passes Board and TransTable to handle state and search.
*/
void UciLoop(Board* board, TransTable* tt);

/*
    ParseUciCommand:
    - Parses a single UCI command line and performs corresponding actions.
*/
void ParseUciCommand(const char* line, Board* board, TransTable* tt);

#endif /* UCI_H */
