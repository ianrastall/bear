//! Castling rights, and the per-game squares castling uses.

use core::ops::{BitAnd, BitOr, Not};

use crate::attacks::between;
use crate::{Bitboard, Color, MoveFlag, Square};

/// Which way the king castles.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum CastleSide {
    King = 0,
    Queen = 1,
}

impl CastleSide {
    pub const ALL: [CastleSide; 2] = [CastleSide::King, CastleSide::Queen];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// File the king ends on: g or c.
    #[inline]
    pub const fn king_to_file(self) -> u8 {
        match self {
            CastleSide::King => 6,
            CastleSide::Queen => 2,
        }
    }

    /// File the rook ends on: f or d.
    #[inline]
    pub const fn rook_to_file(self) -> u8 {
        match self {
            CastleSide::King => 5,
            CastleSide::Queen => 3,
        }
    }

    #[inline]
    pub const fn flag(self) -> MoveFlag {
        match self {
            CastleSide::King => MoveFlag::KingCastle,
            CastleSide::Queen => MoveFlag::QueenCastle,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Default, Debug)]
#[repr(transparent)]
pub struct CastlingRights(u8);

impl CastlingRights {
    pub const NONE: CastlingRights = CastlingRights(0);
    pub const WHITE_KING: CastlingRights = CastlingRights(1);
    pub const WHITE_QUEEN: CastlingRights = CastlingRights(2);
    pub const BLACK_KING: CastlingRights = CastlingRights(4);
    pub const BLACK_QUEEN: CastlingRights = CastlingRights(8);
    pub const ALL: CastlingRights = CastlingRights(15);

    /// Number of distinct values, for indexing Zobrist tables.
    pub const COUNT: usize = 16;

    /// The rights for a raw value; only the low four bits are used.
    #[inline]
    pub const fn from_bits(bits: u8) -> CastlingRights {
        CastlingRights(bits & 15)
    }

    #[inline]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn king_side(color: Color) -> CastlingRights {
        match color {
            Color::White => Self::WHITE_KING,
            Color::Black => Self::BLACK_KING,
        }
    }

    #[inline]
    pub const fn queen_side(color: Color) -> CastlingRights {
        match color {
            Color::White => Self::WHITE_QUEEN,
            Color::Black => Self::BLACK_QUEEN,
        }
    }

    /// The single right for `color` castling to `side`.
    #[inline]
    pub const fn of(color: Color, side: CastleSide) -> CastlingRights {
        match side {
            CastleSide::King => Self::king_side(color),
            CastleSide::Queen => Self::queen_side(color),
        }
    }

    /// Both rights of `color`.
    #[inline]
    pub const fn both(color: Color) -> CastlingRights {
        CastlingRights(Self::king_side(color).0 | Self::queen_side(color).0)
    }

    /// Whether every right in `other` is present.
    #[inline]
    pub const fn contains(self, other: CastlingRights) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any right in `other` is present.
    #[inline]
    pub const fn intersects(self, other: CastlingRights) -> bool {
        self.0 & other.0 != 0
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for CastlingRights {
    type Output = CastlingRights;
    #[inline]
    fn bitor(self, rhs: CastlingRights) -> CastlingRights {
        CastlingRights(self.0 | rhs.0)
    }
}

impl BitAnd for CastlingRights {
    type Output = CastlingRights;
    #[inline]
    fn bitand(self, rhs: CastlingRights) -> CastlingRights {
        CastlingRights(self.0 & rhs.0)
    }
}

impl Not for CastlingRights {
    type Output = CastlingRights;
    #[inline]
    fn not(self) -> CastlingRights {
        CastlingRights(!self.0 & 15)
    }
}

/// Where the castling kings and rooks start, which is fixed for a whole game.
///
/// It is kept outside [`Position`](crate::Position) so that it is not copied with
/// every move. Standard chess and Chess960 differ only in these squares.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CastlingInfo {
    /// Rights that survive a move touching each square.
    rights_mask: [CastlingRights; 64],
    /// Starting square of the castling rook, by colour and side.
    rook_from: [[Option<Square>; 2]; 2],
    /// Squares that must be empty, by colour and side.
    empty_path: [[Bitboard; 2]; 2],
    /// Squares the king crosses or lands on, which must not be attacked.
    king_path: [[Bitboard; 2]; 2],
}

impl CastlingInfo {
    /// No castling possible for either side.
    pub fn none() -> CastlingInfo {
        CastlingInfo {
            rights_mask: [CastlingRights::ALL; 64],
            rook_from: [[None; 2]; 2],
            empty_path: [[Bitboard::EMPTY; 2]; 2],
            king_path: [[Bitboard::EMPTY; 2]; 2],
        }
    }

    /// Records that `color` may castle with the king on `king` and the rook on `rook`,
    /// which must be on the same rank. Returns the side this is.
    pub fn add(&mut self, color: Color, king: Square, rook: Square) -> CastleSide {
        debug_assert!(king.rank() == rook.rank() && king != rook);
        let side = if rook.file() > king.file() {
            CastleSide::King
        } else {
            CastleSide::Queen
        };
        let king_to = Square::from_file_rank(side.king_to_file(), king.rank());
        let rook_to = Square::from_file_rank(side.rook_to_file(), king.rank());

        self.rights_mask[king.index()] =
            self.rights_mask[king.index()] & !CastlingRights::both(color);
        self.rights_mask[rook.index()] =
            self.rights_mask[rook.index()] & !CastlingRights::of(color, side);

        let (c, s) = (color.index(), side.index());
        self.rook_from[c][s] = Some(rook);
        self.king_path[c][s] = between(king, king_to) | king_to.bb();
        self.empty_path[c][s] = (self.king_path[c][s] | between(rook, rook_to) | rook_to.bb())
            & !(king.bb() | rook.bb());
        side
    }

    /// Rights that remain after a move from or to `sq`.
    #[inline]
    pub fn rights_mask(&self, sq: Square) -> CastlingRights {
        self.rights_mask[sq.index()]
    }

    #[inline]
    pub fn rook_from(&self, color: Color, side: CastleSide) -> Option<Square> {
        self.rook_from[color.index()][side.index()]
    }

    /// Squares that must be empty for `color` to castle on `side`.
    #[inline]
    pub fn empty_path(&self, color: Color, side: CastleSide) -> Bitboard {
        self.empty_path[color.index()][side.index()]
    }

    /// Squares that must not be attacked for `color` to castle on `side`.
    #[inline]
    pub fn king_path(&self, color: Color, side: CastleSide) -> Bitboard {
        self.king_path[color.index()][side.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition() {
        let white = CastlingRights::both(Color::White);
        let black = CastlingRights::both(Color::Black);
        assert_eq!(white | black, CastlingRights::ALL);
        assert_eq!(white & black, CastlingRights::NONE);
        assert!(white.contains(CastlingRights::WHITE_KING));
        assert!(white.contains(CastlingRights::WHITE_QUEEN));
        assert!(!white.contains(CastlingRights::BLACK_KING));
        assert!(!white.contains(CastlingRights::ALL));
        assert!(CastlingRights::ALL.intersects(black));
        assert!(!white.intersects(black));
        assert!(CastlingRights::NONE.is_empty());
        assert_eq!(CastlingRights::default(), CastlingRights::NONE);
        assert_eq!(
            CastlingRights::of(Color::Black, CastleSide::Queen),
            CastlingRights::BLACK_QUEEN
        );
    }

    #[test]
    fn removal_stays_in_range() {
        let rights = CastlingRights::ALL & !CastlingRights::both(Color::White);
        assert_eq!(rights, CastlingRights::both(Color::Black));
        assert_eq!((!CastlingRights::NONE).bits(), 15);
        assert_eq!(CastlingRights::from_bits(0xFF), CastlingRights::ALL);
        assert!(CastlingRights::ALL.index() < CastlingRights::COUNT);
    }

    #[test]
    fn standard_squares() {
        let mut info = CastlingInfo::none();
        assert_eq!(
            info.add(Color::White, Square::E1, Square::H1),
            CastleSide::King
        );
        assert_eq!(
            info.add(Color::White, Square::E1, Square::A1),
            CastleSide::Queen
        );

        assert_eq!(
            info.rook_from(Color::White, CastleSide::King),
            Some(Square::H1)
        );
        assert_eq!(info.rook_from(Color::Black, CastleSide::King), None);
        assert_eq!(
            info.empty_path(Color::White, CastleSide::King),
            Square::F1.bb() | Square::G1.bb()
        );
        assert_eq!(
            info.empty_path(Color::White, CastleSide::Queen),
            Square::B1.bb() | Square::C1.bb() | Square::D1.bb()
        );
        assert_eq!(
            info.king_path(Color::White, CastleSide::Queen),
            Square::C1.bb() | Square::D1.bb()
        );

        assert_eq!(
            info.rights_mask(Square::E1),
            CastlingRights::both(Color::Black)
        );
        assert_eq!(info.rights_mask(Square::H1), !CastlingRights::WHITE_KING);
        assert_eq!(info.rights_mask(Square::E4), CastlingRights::ALL);
    }

    #[test]
    fn chess960_squares_where_the_pieces_barely_move() {
        // King on g1 and rook on h1: the king stays put and the rook crosses it.
        let mut info = CastlingInfo::none();
        info.add(Color::White, Square::G1, Square::H1);
        assert_eq!(
            info.king_path(Color::White, CastleSide::King),
            Square::G1.bb()
        );
        assert_eq!(
            info.empty_path(Color::White, CastleSide::King),
            Square::F1.bb()
        );
    }
}
