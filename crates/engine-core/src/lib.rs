//! Core of the Bear chess engine.
//!
//! This crate holds the board representation, attack tables, move generation and perft,
//! and will hold the search. See `docs/architecture.md` for the design.
//!
//! `unsafe` is denied crate-wide and allowed only at the sites listed in the
//! architecture document, each with a `SAFETY` comment.

#![deny(unsafe_code)]

pub mod attacks;
pub mod bitboard;
pub mod castling;
pub mod chess_move;
pub mod movegen;
pub mod perft;
pub mod piece;
pub mod position;
pub mod square;
pub mod zobrist;

pub use bitboard::Bitboard;
pub use castling::{CastleSide, CastlingInfo, CastlingRights};
pub use chess_move::{Move, MoveFlag};
pub use movegen::{MoveList, ScoredMove};
pub use piece::{Color, Piece, PieceType};
pub use position::{BoardState, FenError, Position, START_FEN};
pub use square::Square;
