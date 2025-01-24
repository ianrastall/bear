/****************************************************************************
 * File: uci.h
 ****************************************************************************/
/*
   Description:
   - Header for handling UCI (Universal Chess Interface) commands and loop.
   - Declares functions to read commands from stdin, parse them, and
     execute corresponding engine actions (like setting positions, starting
     a search, etc.).

   Exports:
   1) void UciLoop(void);
   2) void ParseUciCommand(const char* line);
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

/*
   UciLoop:
   - Main loop reading lines from stdin until "quit".
   - Each line is handed to ParseUciCommand().

   ParseUciCommand:
   - Handles known UCI commands like "uci", "isready", "position", "go", "stop".
   - For each command, we do the appropriate engine action (e.g. set FEN, start search).
*/

void UciLoop(void);
void ParseUciCommand(const char* line);

#endif /* UCI_H */
