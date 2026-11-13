//! Core of the Bear chess engine.
//!
//! This crate holds the primitive types now and will hold attack tables, the position,
//! move generation and the search. See `docs/architecture.md` for the design.
//!
//! `unsafe` is denied crate-wide and allowed only at the sites listed in the
//! architecture document, each with a `SAFETY` comment.

#![deny(unsafe_code)]

pub mod bitboard;
pub mod castling;
pub mod chess_move;
pub mod piece;
pub mod square;

pub use bitboard::Bitboard;
pub use castling::CastlingRights;
pub use chess_move::{Move, MoveFlag};
pub use piece::{Color, Piece, PieceType};
pub use square::Square;
