/****************************************************************************
 * File: log.h
 ****************************************************************************/
/*
    Description:
    - Header for centralized logging functionality.
    - Provides functions to initialize logging, set log levels, and log messages.
*/

#ifndef LOG_H
#define LOG_H

#include <stdio.h>
#include <stdarg.h>
#include <stdbool.h>

/* Logging levels */
typedef enum {
    LOG_INFO,
    LOG_DEBUG,
    LOG_WARN,
    LOG_ERROR
} LogLevel;

/* Initialize logging with debug mode */
void InitLogging(bool debug);

/* Set the current logging level */
void SetLogLevel(LogLevel level);

/* Logging function with level */
void LogMessage(LogLevel level, const char* format, ...);

#endif /* LOG_H */
