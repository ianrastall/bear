/****************************************************************************
 * File: board.c
 ****************************************************************************/
/*
   Description:
   - Implementation of board-related functions.
   - Initialize or reset the board.
   - Set up from a FEN string.
   - Print or debug the board if needed.
   - Maintain any auxiliary data structures (e.g., piece lists, bitboards).
*/

#include "board.h"
#include <ctype.h>  /* For isdigit, isalpha */
#include <string.h> /* For strtok, strchr, etc. */

/* 
   Helper macros for castling rights. 
   These are just an example bit scheme:
   1 = White can castle kingside
   2 = White can castle queenside
   4 = Black can castle kingside
   8 = Black can castle queenside
*/
#define WKCA 1
#define WQCA 2
#define BKCA 4
#define BQCA 8

/*
   Convert 0-based file and rank into 120-based mailbox index.
   If file in [0..7] and rank in [0..7], 
   then the 120-based index is (rank+2)*10 + (file+1).
*/
static int FRTo120(int file, int rank) {
    return (rank + 2) * 10 + (file + 1);
}

/*
   A small helper to return the correct piece code (W_PAWN, B_QUEEN, etc.)
   given a character from the FEN (e.g. 'P', 'k', etc.).
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

/*
   Resets the board to an empty or initial state. 
*/
void InitBoard(Board* b)
{
    int i;
    for (i = 0; i < BOARD_SIZE; i++) {
        b->pieces[i] = EMPTY;
    }
    b->side = WHITE;
    b->enPas = -1;
    b->fiftyMove = 0;
    b->ply = 0;
    b->hisPly = 0;
    b->castlePerm = 0;
    b->positionKey = 0ULL;
    /* Additional initialization logic can go here if needed. */
}

/*
   Sets the board from a given FEN string.

   A typical FEN has the format:
   rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1

   That is broken down as:
   1) Piece placement (from 8th rank to 1st rank)
   2) Side to move ("w" or "b")
   3) Castling availability (e.g. "KQkq" or "-")
   4) En passant target square (e.g. "e3" or "-")
   5) Halfmove clock (for 50 move rule)
   6) Fullmove number
*/
void SetFen(Board* b, const char* fen)
{
    InitBoard(b);

    if (fen == NULL || strlen(fen) < 1) {
        /* If FEN is invalid or empty, just leave it as init. */
        return;
    }

    /* We'll make a local buffer to safely tokenize. */
    char fenCopy[256];
    strncpy(fenCopy, fen, 255);
    fenCopy[255] = '\0';

    /* 1) Piece placement */
    char* token = strtok(fenCopy, " ");
    if (token == NULL) return;
    {
        int rank = 7;
        int file = 0;
        const char* ptr = token;

        while (*ptr && rank >= 0) {
            if (isdigit((unsigned char)*ptr)) {
                /* e.g., '2' means 2 empty squares */
                int emptyCount = *ptr - '0';
                file += emptyCount;
                ptr++;
            } else if (*ptr == '/') {
                /* next rank */
                rank--;
                file = 0;
                ptr++;
            } else {
                /* It's a piece character (like 'p', 'Q', etc.) */
                int piece = CharToPiece(*ptr);
                ptr++;
                if (piece != EMPTY && file < 8) {
                    int sq120 = FRTo120(file, rank);
                    b->pieces[sq120] = piece;
                    file++;
                }
            }
        }
    }

    /* 2) Side to move */
    token = strtok(NULL, " ");
    if (token == NULL) return;
    {
        b->side = (*token == 'w' || *token == 'W') ? WHITE : BLACK;
    }

    /* 3) Castling availability */
    token = strtok(NULL, " ");
    if (token == NULL) return;
    {
        b->castlePerm = 0;
        if (*token != '-') {
            if (strchr(token, 'K')) b->castlePerm |= WKCA;
            if (strchr(token, 'Q')) b->castlePerm |= WQCA;
            if (strchr(token, 'k')) b->castlePerm |= BKCA;
            if (strchr(token, 'q')) b->castlePerm |= BQCA;
        }
    }

    /* 4) En passant target square */
    token = strtok(NULL, " ");
    if (token == NULL) return;
    {
        if (*token == '-') {
            b->enPas = -1; 
        } else {
            int file = token[0] - 'a';
            int rank = token[1] - '1';
            b->enPas = FRTo120(file, rank);
        }
    }

    /* 5) Halfmove clock (for the 50-move rule) */
    token = strtok(NULL, " ");
    if (token == NULL) return;
    {
        b->fiftyMove = atoi(token);
    }

    /* 6) Fullmove number */
    token = strtok(NULL, " ");
    if (token == NULL) return;
    {
        /* We usually don't store fullmove in the Board struct, but if you want: */
        b->hisPly = (atoi(token) - 1) * 2; 
        /* Rough approach: if it's White to move, that's 2*N halfmoves done. 
           If it's Black to move, 2*N+1 halfmoves done. But let's keep it simple. */
        if (b->side == BLACK) {
            b->hisPly++;
        }
    }

    /* 
       If you’re using Zobrist keys, you’d compute/update b->positionKey 
       here based on piece placement, side, castling, and en passant. 
       For now, we’ll leave that out (or you could call a function like 
       b->positionKey = GeneratePositionKey(b);
    */
}

/*
   Prints the board to stdout in a simple textual format. 
   You can customize the style to taste.
*/
void PrintBoard(const Board* b)
{
    int rank, file;
    for (rank = 7; rank >= 0; rank--) {
        printf("%d  ", rank + 1);
        for (file = 0; file < 8; file++) {
            int sq120 = FRTo120(file, rank);
            int piece = b->pieces[sq120];
            switch(piece) {
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
    printf("Side to move: %s\n", (b->side == WHITE) ? "WHITE" : "BLACK");

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
        int file = (b->enPas % 10) - 1;   /* reverse of FRTo120 */
        int rank = (b->enPas / 10) - 2;
        printf("En Passant: %c%d\n", 'a' + file, rank + 1);
    } else {
        printf("En Passant: -\n");
    }

    /* Halfmove clock and Ply info */
    printf("Halfmove (50-move) clock: %d\n", b->fiftyMove);
    printf("Ply: %d, hisPly: %d\n", b->ply, b->hisPly);

    /* Position Key (if used) */
    printf("Position Key: %llu\n", (unsigned long long) b->positionKey);

    printf("\n");
}
