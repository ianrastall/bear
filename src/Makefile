#############################################################################
# Bear Chess Engine - Makefile
#############################################################################
# This Makefile assumes all .c and .h are in the same directory.
# Adjust compiler, flags, or file names if needed.

# The compiler you want to use
CC      = gcc

# Compiler flags:
#  -Wall enables common warnings
#  -O2   applies a decent optimization level
CFLAGS  = -Wall -O2

# List all source files
SOURCES = \
    main.c \
    board.c \
    evaluate.c \
    movegen.c \
    search.c \
    transposition.c \
    uci.c

# Generate a list of object files by replacing .c with .o
OBJECTS = $(SOURCES:.c=.o)

# The final executable name
TARGET  = bear

#############################################################################
# Default rule: build the Bear engine
#############################################################################
all: $(TARGET)

# Link step: combine all object files into the final executable
$(TARGET): $(OBJECTS)
	$(CC) $(CFLAGS) $(OBJECTS) -o $(TARGET)

# Compilation rule: compile each .c into a .o
# $< is the source, $@ is the target
%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

# Optional cleanup rule
clean:
	rm -f $(OBJECTS) $(TARGET)

# If you'd like to run after building (on Linux/macOS), you can do:
# make && ./bear
#
# On Windows (MinGW), you'd run:
# make && bear.exe

