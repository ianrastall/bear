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

void UciLoop(Board* board, TransTable* tt);
void ParseUciCommand(const char* line, Board* board, TransTable* tt);

#endif /* UCI_H */
