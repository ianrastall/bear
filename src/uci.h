/****************************************************************************
 * File: uci.h
 ****************************************************************************/
/*
   Description:
   - Header for the UCI (Universal Chess Interface) protocol handling.
   - Declares functions to process UCI commands (position, go, stop, quit).
   - In a modern engine, UCI is standard for communicating with GUIs.

   Functions to declare here:
   1) void UciLoop(void);
   2) void ParseUciCommand(const char* line);
   3) etc.
*/

#ifndef UCI_H
#define UCI_H

/* Include standard headers and other engine modules as needed */
#include <stdio.h>
#include <string.h>
#include "board.h"
#include "search.h"

void UciLoop(void);
void ParseUciCommand(const char* line);

#endif /* UCI_H */
