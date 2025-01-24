/****************************************************************************
 * File: log.c
 ****************************************************************************/
/*
   Implementation of the centralized logging functionality.
   Controls what messages are printed based on the current log level and debug mode.
*/

#include "log.h"

static bool isDebug = false;
static LogLevel currentLevel = LOG_INFO;
static FILE* logFile = NULL; /* Optional: can be extended to log to files */

void InitLogging(bool debug)
{
    isDebug = debug;
}

void SetLogLevel(LogLevel level)
{
    currentLevel = level;
}

void LogMessage(LogLevel level, const char* format, ...)
{
    if(level < currentLevel) return;

    va_list args;
    va_start(args, format);

    /* Prepare the message prefix */
    char buffer[1024] = {0};
    switch(level) {
        case LOG_INFO:
            strcat(buffer, "[INFO] ");
            break;
        case LOG_DEBUG:
            if(!isDebug) return; /* Only log DEBUG if debug is enabled */
            strcat(buffer, "[DEBUG] ");
            break;
        case LOG_WARN:
            strcat(buffer, "[WARN] ");
            break;
        case LOG_ERROR:
            strcat(buffer, "[ERROR] ");
            break;
    }

    /* Append the formatted message */
    vsnprintf(buffer + strlen(buffer), sizeof(buffer) - strlen(buffer), format, args);

    /* Output to logFile or stdout */
    if(logFile) {
        fprintf(logFile, "%s", buffer);
        fflush(logFile);
    } else {
        printf("%s", buffer);
    }

    va_end(args);
}
