//! Moves packed into 16 bits.
//!
//! ```text
//! bits 0–5 from | bits 6–11 to | bits 12–15 flag
//! ```
//!
//! Castling is encoded as the king moving onto its own rook, which represents
//! Chess960 castling without ambiguity. [`Move::uci`] converts to the king's
//! destination square for standard chess.

use core::fmt;
use core::num::NonZeroU16;

use crate::{PieceType, Square};

/// What kind of move this is. The discriminant is the four-bit flag field.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum MoveFlag {
    Quiet = 0,
    DoublePush = 1,
    KingCastle = 2,
    QueenCastle = 3,
    Capture = 4,
    EnPassant = 5,
    PromoKnight = 8,
    PromoBishop = 9,
    PromoRook = 10,
    PromoQueen = 11,
    PromoCaptureKnight = 12,
    PromoCaptureBishop = 13,
    PromoCaptureRook = 14,
    PromoCaptureQueen = 15,
}

impl MoveFlag {
    pub const ALL: [MoveFlag; 14] = [
        MoveFlag::Quiet,
        MoveFlag::DoublePush,
        MoveFlag::KingCastle,
        MoveFlag::QueenCastle,
        MoveFlag::Capture,
        MoveFlag::EnPassant,
        MoveFlag::PromoKnight,
        MoveFlag::PromoBishop,
        MoveFlag::PromoRook,
        MoveFlag::PromoQueen,
        MoveFlag::PromoCaptureKnight,
        MoveFlag::PromoCaptureBishop,
        MoveFlag::PromoCaptureRook,
        MoveFlag::PromoCaptureQueen,
    ];
}

const FLAG_SHIFT: u16 = 12;
const CAPTURE_BIT: u16 = 1 << 14;
const PROMOTION_BIT: u16 = 1 << 15;
const PROMOTION_PIECES: [PieceType; 4] = [
    PieceType::Knight,
    PieceType::Bishop,
    PieceType::Rook,
    PieceType::Queen,
];

/// A move. `Option<Move>` is two bytes: the all-zero pattern (a1 to a1, quiet) is never a move.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Move(NonZeroU16);

const _: () = assert!(size_of::<Option<Move>>() == 2);

impl Move {
    /// Builds a move. `from` and `to` must differ.
    #[inline]
    pub const fn new(from: Square, to: Square, flag: MoveFlag) -> Move {
        debug_assert!(from as u8 != to as u8);
        let raw = from as u16 | ((to as u16) << 6) | ((flag as u16) << FLAG_SHIFT);
        match NonZeroU16::new(raw) {
            Some(raw) => Move(raw),
            None => panic!("a1 to a1 is not a move"),
        }
    }

    /// Rebuilds a move from its 16-bit form, as stored in the transposition table.
    ///
    /// Only the all-zero pattern is rejected. Any other value yields a `Move`, which
    /// may not be playable; the position must validate it before use.
    #[inline]
    pub const fn from_raw(raw: u16) -> Option<Move> {
        match NonZeroU16::new(raw) {
            Some(raw) => Some(Move(raw)),
            None => None,
        }
    }

    #[inline]
    pub const fn raw(self) -> u16 {
        self.0.get()
    }

    #[inline]
    pub const fn from(self) -> Square {
        Square::from_index((self.raw() & 63) as u8)
    }

    /// Destination square. For castling this is the rook's square.
    #[inline]
    pub const fn to(self) -> Square {
        Square::from_index(((self.raw() >> 6) & 63) as u8)
    }

    /// The four-bit flag field.
    #[inline]
    pub const fn flag_bits(self) -> u16 {
        self.raw() >> FLAG_SHIFT
    }

    /// Captures, including en passant and promotion captures.
    #[inline]
    pub const fn is_capture(self) -> bool {
        self.raw() & CAPTURE_BIT != 0
    }

    #[inline]
    pub const fn is_promotion(self) -> bool {
        self.raw() & PROMOTION_BIT != 0
    }

    /// The piece promoted to, if this is a promotion.
    #[inline]
    pub const fn promotion(self) -> Option<PieceType> {
        if self.is_promotion() {
            Some(PROMOTION_PIECES[(self.flag_bits() & 3) as usize])
        } else {
            None
        }
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        self.flag_bits() >> 1 == 1
    }

    #[inline]
    pub const fn is_king_castle(self) -> bool {
        self.flag_bits() == MoveFlag::KingCastle as u16
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        self.flag_bits() == MoveFlag::EnPassant as u16
    }

    #[inline]
    pub const fn is_double_push(self) -> bool {
        self.flag_bits() == MoveFlag::DoublePush as u16
    }

    /// The move in UCI notation.
    ///
    /// With `chess960` false, castling is written as the king's two-square move
    /// (`e1g1`). With it true, castling is written as king takes rook (`e1h1`).
    #[inline]
    pub const fn uci(self, chess960: bool) -> UciMove {
        UciMove { mv: self, chess960 }
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Move({}{}, flag {})",
            self.from(),
            self.to(),
            self.flag_bits()
        )
    }
}

/// Displays a move in UCI notation. Created by [`Move::uci`].
#[derive(Copy, Clone)]
pub struct UciMove {
    mv: Move,
    chess960: bool,
}

impl fmt::Display for UciMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let from = self.mv.from();
        let to = if self.mv.is_castle() && !self.chess960 {
            let file = if self.mv.is_king_castle() { 6 } else { 2 };
            Square::from_file_rank(file, from.rank())
        } else {
            self.mv.to()
        };
        write!(f, "{from}{to}")?;
        if let Some(piece_type) = self.mv.promotion() {
            write!(f, "{}", piece_type.to_char())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_round_trip() {
        // Every from/to/flag combination, or a sample of the squares under Miri.
        let step = if cfg!(miri) { 7 } else { 1 };
        for from in (0..64u8).step_by(step) {
            for to in (0..64u8).step_by(step) {
                if from == to {
                    continue;
                }
                for flag in MoveFlag::ALL {
                    let mv = Move::new(Square::from_index(from), Square::from_index(to), flag);
                    assert_eq!(mv.from() as u8, from);
                    assert_eq!(mv.to() as u8, to);
                    assert_eq!(mv.flag_bits(), flag as u16);
                    assert_eq!(Move::from_raw(mv.raw()), Some(mv));
                }
            }
        }
    }

    #[test]
    fn zero_is_not_a_move() {
        assert_eq!(Move::from_raw(0), None);
        assert!(Move::from_raw(1).is_some());
    }

    #[test]
    fn predicates() {
        use MoveFlag::*;
        for flag in MoveFlag::ALL {
            let mv = Move::new(Square::E2, Square::E4, flag);
            let capture = matches!(
                flag,
                Capture
                    | EnPassant
                    | PromoCaptureKnight
                    | PromoCaptureBishop
                    | PromoCaptureRook
                    | PromoCaptureQueen
            );
            let promotion = match flag {
                PromoKnight | PromoCaptureKnight => Some(PieceType::Knight),
                PromoBishop | PromoCaptureBishop => Some(PieceType::Bishop),
                PromoRook | PromoCaptureRook => Some(PieceType::Rook),
                PromoQueen | PromoCaptureQueen => Some(PieceType::Queen),
                _ => None,
            };
            assert_eq!(mv.is_capture(), capture, "{flag:?}");
            assert_eq!(mv.promotion(), promotion, "{flag:?}");
            assert_eq!(mv.is_promotion(), promotion.is_some(), "{flag:?}");
            assert_eq!(
                mv.is_castle(),
                matches!(flag, KingCastle | QueenCastle),
                "{flag:?}"
            );
            assert_eq!(mv.is_king_castle(), flag == KingCastle, "{flag:?}");
            assert_eq!(mv.is_en_passant(), flag == EnPassant, "{flag:?}");
            assert_eq!(mv.is_double_push(), flag == DoublePush, "{flag:?}");
        }
    }

    #[test]
    fn uci_notation() {
        let quiet = Move::new(Square::G1, Square::F3, MoveFlag::Quiet);
        assert_eq!(quiet.uci(false).to_string(), "g1f3");
        assert_eq!(quiet.uci(true).to_string(), "g1f3");

        let promo = Move::new(Square::E7, Square::E8, MoveFlag::PromoQueen);
        assert_eq!(promo.uci(false).to_string(), "e7e8q");

        let under = Move::new(Square::B2, Square::A1, MoveFlag::PromoCaptureKnight);
        assert_eq!(under.uci(false).to_string(), "b2a1n");
    }

    #[test]
    fn castling_notation() {
        let white_short = Move::new(Square::E1, Square::H1, MoveFlag::KingCastle);
        let white_long = Move::new(Square::E1, Square::A1, MoveFlag::QueenCastle);
        let black_short = Move::new(Square::E8, Square::H8, MoveFlag::KingCastle);
        let black_long = Move::new(Square::E8, Square::A8, MoveFlag::QueenCastle);

        assert_eq!(white_short.uci(false).to_string(), "e1g1");
        assert_eq!(white_long.uci(false).to_string(), "e1c1");
        assert_eq!(black_short.uci(false).to_string(), "e8g8");
        assert_eq!(black_long.uci(false).to_string(), "e8c8");

        assert_eq!(white_short.uci(true).to_string(), "e1h1");
        assert_eq!(white_long.uci(true).to_string(), "e1a1");
        assert_eq!(black_short.uci(true).to_string(), "e8h8");
        assert_eq!(black_long.uci(true).to_string(), "e8a8");
    }
}
