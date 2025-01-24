/****************************************************************************
 * File: board.c
 ****************************************************************************/
/*
   Description:
   - Implementation of board-related functions.
*/

#include "board.h"
#include <ctype.h>  /* For isdigit() */
#include <string.h> /* For strtok(), strncpy(), etc. */

static int FRTo120(int file, int rank) {
    return (rank + 2) * 10 + (file + 1);
}

/* 
   Helper: Convert a single FEN character to our piece ID (W_PAWN..B_KING).
   Returns EMPTY if it doesn't match a known piece.
*/
static int CharToPiece(char c) {
    switch (c) {
        case 'P': return W_PAWN;
        case 'N': return W_KNIGHT;
        case 'B': return W_BISHOP;
        case 'R': return W_ROOK;
        case 'Q': return W_QUEEN;
        case 'K': return W_KING;
        case 'p': return B_PAWN;
        case 'n': return B_KNIGHT;
        case 'b': return B_BISHOP;
        case 'r': return B_ROOK;
        case 'q': return B_QUEEN;
        case 'k': return B_KING;
    }
    return EMPTY;
}

/* Reset the board to an empty or initial state. */
void InitBoard(Board* b) {
    for (int i = 0; i < BOARD_SIZE; i++) {
        b->pieces[i] = EMPTY;
    }
    b->side = WHITE;
    b->enPas = -1;
    b->fiftyMove = 0;
    b->ply = 0;
    b->hisPly = 0;
    b->castlePerm = 0;
    b->positionKey = 0ULL;
    /* Any other init logic can go here. */
}

/* Set the board from a FEN string. */
void SetFen(Board* b, const char* fen) {
    InitBoard(b);

    if (!fen || strlen(fen) == 0) {
        /* If FEN is invalid or empty, just leave it in init state. */
        return;
    }

    char fenCopy[256];
    strncpy(fenCopy, fen, 255);
    fenCopy[255] = '\0';

    /* 1) Piece placement */
    char* token = strtok(fenCopy, " ");
    if (!token) return;

    {
        int rank = 7;
        int file = 0;
        const char* ptr = token;

        while (*ptr && rank >= 0) {
            if (isdigit((unsigned char)*ptr)) {
                /* e.g. '3' => 3 empty squares */
                file += (*ptr - '0');
                ptr++;
            } else if (*ptr == '/') {
                rank--;
                file = 0;
                ptr++;
            } else {
                int piece = CharToPiece(*ptr);
                ptr++;
                if (piece != EMPTY && file < 8) {
                    b->pieces[FRTo120(file, rank)] = piece;
                    file++;
                }
            }
        }
    }

    /* 2) Side to move */
    token = strtok(NULL, " ");
    if (!token) return;
    b->side = ((*token == 'w' || *token == 'W') ? WHITE : BLACK);

    /* 3) Castling availability */
    token = strtok(NULL, " ");
    if (!token) return;
    b->castlePerm = 0;
    if (*token != '-') {
        if (strchr(token, 'K')) b->castlePerm |= WKCA;
        if (strchr(token, 'Q')) b->castlePerm |= WQCA;
        if (strchr(token, 'k')) b->castlePerm |= BKCA;
        if (strchr(token, 'q')) b->castlePerm |= BQCA;
    }

    /* 4) En passant target square */
    token = strtok(NULL, " ");
    if (!token) return;
    if (*token == '-') {
        b->enPas = -1;
    } else {
        int f = token[0] - 'a';
        int r = token[1] - '1';
        b->enPas = FRTo120(f, r);
    }

    /* 5) Halfmove clock */
    token = strtok(NULL, " ");
    if (!token) return;
    b->fiftyMove = atoi(token);

    /* 6) Fullmove number */
    token = strtok(NULL, " ");
    if (!token) return;
    b->hisPly = (atoi(token) - 1) * 2;
    if (b->side == BLACK) {
        b->hisPly++;
    }

    /* If using Zobrist hashing, update b->positionKey here. */
}

/* Print the board in ASCII format for debugging or console output. */
void PrintBoard(const Board* b) {
    for (int rank = 7; rank >= 0; rank--) {
        printf("%d  ", rank + 1);
        for (int file = 0; file < 8; file++) {
            int sq120 = FRTo120(file, rank);
            int piece = b->pieces[sq120];
            switch (piece) {
                case EMPTY:     printf(". ");  break;
                case W_PAWN:    printf("P ");  break;
                case W_KNIGHT:  printf("N ");  break;
                case W_BISHOP:  printf("B ");  break;
                case W_ROOK:    printf("R ");  break;
                case W_QUEEN:   printf("Q ");  break;
                case W_KING:    printf("K ");  break;
                case B_PAWN:    printf("p ");  break;
                case B_KNIGHT:  printf("n ");  break;
                case B_BISHOP:  printf("b ");  break;
                case B_ROOK:    printf("r ");  break;
                case B_QUEEN:   printf("q ");  break;
                case B_KING:    printf("k ");  break;
                default:        printf("? ");  break;
            }
        }
        printf("\n");
    }
    printf("\n   a b c d e f g h\n\n");

    /* Side to move */
    printf("Side to move: %s\n", b->side == WHITE ? "WHITE" : "BLACK");

    /* Castling permissions */
    printf("Castling rights: ");
    if (b->castlePerm & WKCA) printf("K");
    if (b->castlePerm & WQCA) printf("Q");
    if (b->castlePerm & BKCA) printf("k");
    if (b->castlePerm & BQCA) printf("q");
    if (!b->castlePerm) printf("-");
    printf("\n");

    /* En passant square */
    if (b->enPas != -1) {
        int file = (b->enPas % 10) - 1;
        int rankEP = (b->enPas / 10) - 2;
        printf("En Passant: %c%d\n", 'a' + file, rankEP + 1);
    } else {
        printf("En Passant: -\n");
    }

    /* Halfmove clock and Ply info */
    printf("Halfmove (50-move) clock: %d\n", b->fiftyMove);
    printf("Ply: %d, hisPly: %d\n", b->ply, b->hisPly);

    /* Position Key (if used) */
    printf("Position Key: %llu\n", (unsigned long long)b->positionKey);

    printf("\n");
}
