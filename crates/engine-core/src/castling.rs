//! Castling rights as a four-bit set.

use core::ops::{BitAnd, BitOr, Not};

use crate::Color;

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
    }

    #[test]
    fn removal_stays_in_range() {
        let rights = CastlingRights::ALL & !CastlingRights::both(Color::White);
        assert_eq!(rights, CastlingRights::both(Color::Black));
        assert_eq!((!CastlingRights::NONE).bits(), 15);
        assert_eq!(CastlingRights::from_bits(0xFF), CastlingRights::ALL);
        assert!(CastlingRights::ALL.index() < CastlingRights::COUNT);
    }
}
