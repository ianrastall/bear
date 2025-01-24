/****************************************************************************
 * File: board.c
 ****************************************************************************/
/*
    Implementation of board-related functions.
*/

#include "board.h"
#include <ctype.h>  /* For isdigit() */
#include <string.h> /* For strtok(), strncpy(), etc. */
#include <stdio.h>  /* For logging */

/* Converts file (0-7) and rank (0-7) to 120-based index */
int FRTo120(int file, int rank) {
    return (rank + 2) * 10 + (file + 1);
}

/* Initialize the board to the standard starting position */
void InitBoard(Board* b)
{
    /* Initialize all squares to EMPTY */
    for(int i = 0; i < BOARD_SIZE; i++) {
        b->pieces[i] = EMPTY;
    }

    /* Set up white pieces */
    b->pieces[FRTo120(0,0)] = W_ROOK;
    b->pieces[FRTo120(1,0)] = W_KNIGHT;
    b->pieces[FRTo120(2,0)] = W_BISHOP;
    b->pieces[FRTo120(3,0)] = W_QUEEN;
    b->pieces[FRTo120(4,0)] = W_KING;
    b->pieces[FRTo120(5,0)] = W_BISHOP;
    b->pieces[FRTo120(6,0)] = W_KNIGHT;
    b->pieces[FRTo120(7,0)] = W_ROOK;
    for(int f = 0; f < 8; f++) {
        b->pieces[FRTo120(f,1)] = W_PAWN;
    }

    /* Set up black pieces */
    b->pieces[FRTo120(0,7)] = B_ROOK;
    b->pieces[FRTo120(1,7)] = B_KNIGHT;
    b->pieces[FRTo120(2,7)] = B_BISHOP;
    b->pieces[FRTo120(3,7)] = B_QUEEN;
    b->pieces[FRTo120(4,7)] = B_KING;
    b->pieces[FRTo120(5,7)] = B_BISHOP;
    b->pieces[FRTo120(6,7)] = B_KNIGHT;
    b->pieces[FRTo120(7,7)] = B_ROOK;
    for(int f = 0; f < 8; f++) {
        b->pieces[FRTo120(f,6)] = B_PAWN;
    }

    /* Set side to move */
    b->side = WHITE;

    /* No en passant square */
    b->enPas = EMPTY;

    /* All castling rights available */
    b->castlePerm = WKCA | WQCA | BKCA | BQCA;

    LogMessage(LOG_DEBUG, "Board initialized to standard starting position.\n");
}

/* Set the board position based on a FEN string */
void SetFen(Board* b, const char* fen, bool debugMode)
{
    /* Reset the board first */
    InitBoard(b);

    /* Example FEN parsing logic */
    /* Split FEN into fields: piece placement, active color, castling availability, en passant, halfmove clock, fullmove number */

    char copy[1024];
    strncpy(copy, fen, sizeof(copy));
    copy[sizeof(copy)-1] = '\0'; /* Ensure null-termination */

    char* token = strtok(copy, " ");
    if(!token) return;

    /* Piece placement */
    int rank = 7; /* Start from rank 8 */
    int file = 0;
    for(int i = 0; i < strlen(token) && rank >=0; i++) {
        char c = token[i];
        if(c == '/') {
            rank--;
            file = 0;
            continue;
        }
        if(isdigit(c)) {
            int emptySquares = c - '0';
            for(int e = 0; e < emptySquares; e++) {
                b->pieces[FRTo120(file, rank)] = EMPTY;
                file++;
            }
        }
        else {
            int piece = CharToPiece(c);
            b->pieces[FRTo120(file, rank)] = piece;
            file++;
        }
    }

    /* Active color */
    token = strtok(NULL, " ");
    if(token && token[0] == 'b') {
        b->side = BLACK;
    }
    else {
        b->side = WHITE;
    }

    /* Castling availability */
    token = strtok(NULL, " ");
    b->castlePerm = 0;
    if(token) {
        for(int i = 0; i < strlen(token); i++) {
            switch(token[i]) {
                case 'K': b->castlePerm |= WKCA; break;
                case 'Q': b->castlePerm |= WQCA; break;
                case 'k': b->castlePerm |= BKCA; break;
                case 'q': b->castlePerm |= BQCA; break;
                case '-': /* No castling */ break;
                default:
                    LogMessage(LOG_WARN, "Unknown castling character: %c\n", token[i]);
                    break;
            }
        }
    }

    /* En passant target square */
    token = strtok(NULL, " ");
    if(token && token[0] != '-') {
        int file = token[0] - 'a';
        int rank = token[1] - '1';
        b->enPas = FRTo120(file, rank);
    }
    else {
        b->enPas = EMPTY;
    }

    /* Halfmove clock and fullmove number are ignored for now */

    LogMessage(LOG_DEBUG, "Board set from FEN: %s\n", fen);
}

/* Convert character to piece code */
int CharToPiece(char c) {
    switch(tolower(c)) {
        case 'p': return (islower(c) ? B_PAWN : W_PAWN);
        case 'n': return (islower(c) ? B_KNIGHT : W_KNIGHT);
        case 'b': return (islower(c) ? B_BISHOP : W_BISHOP);
        case 'r': return (islower(c) ? B_ROOK : W_ROOK);
        case 'q': return (islower(c) ? B_QUEEN : W_QUEEN);
        case 'k': return (islower(c) ? B_KING : W_KING);
        default: return EMPTY;
    }
}

/* Placeholder: Implement a proper move legality check */
bool IsMoveLegal(Board* b, Move move) {
    /* Implement your move legality logic here */
    /* For now, assume all moves are legal */
    return true;
}

/* Placeholder: Implement making a move on the board */
void MakeMove(Board* b, Move move) {
    /* Implement your move making logic here */
    /* Example: Move the piece */
    b->pieces[move.to] = b->pieces[move.from];
    b->pieces[move.from] = EMPTY;

    /* Handle promotions */
    if(move.flag & (1 << 3)) { /* Promotion flag */
        b->pieces[move.to] = move.promoted;
    }

    /* Toggle side */
    b->side = (b->side == WHITE) ? BLACK : WHITE;

    /* Handle en passant, castling rights, etc. */
    /* Implement your logic here */

    LogMessage(LOG_DEBUG, "Moved piece from %d to %d.\n", move.from, move.to);
}
