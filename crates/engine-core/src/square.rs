//! Board squares in little-endian rank-file order: a1 = 0, b1 = 1, …, h8 = 63.

use core::fmt;
use core::str::FromStr;

use crate::Bitboard;

#[rustfmt::skip]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Square {
    A1, B1, C1, D1, E1, F1, G1, H1,
    A2, B2, C2, D2, E2, F2, G2, H2,
    A3, B3, C3, D3, E3, F3, G3, H3,
    A4, B4, C4, D4, E4, F4, G4, H4,
    A5, B5, C5, D5, E5, F5, G5, H5,
    A6, B6, C6, D6, E6, F6, G6, H6,
    A7, B7, C7, D7, E7, F7, G7, H7,
    A8, B8, C8, D8, E8, F8, G8, H8,
}

// `from_index` relies on the discriminants being exactly 0..=63.
const _: () = assert!(Square::A1 as u8 == 0 && Square::H8 as u8 == 63);
const _: () = assert!(size_of::<Option<Square>>() == 1);

impl Square {
    pub const COUNT: usize = 64;

    /// The square with index `i`. `i` must be below 64; only the low six bits are used.
    #[inline]
    #[allow(unsafe_code)]
    pub const fn from_index(i: u8) -> Square {
        debug_assert!(i < 64);
        // SAFETY: the value is masked to 0..=63, and `Square` is `repr(u8)` with exactly
        // the discriminants 0..=63 (const-asserted above), so every masked value is valid.
        unsafe { core::mem::transmute::<u8, Square>(i & 63) }
    }

    /// The square with index `i`, or `None` if `i` is not below 64.
    #[inline]
    pub const fn try_from_index(i: u8) -> Option<Square> {
        if i < 64 {
            Some(Square::from_index(i))
        } else {
            None
        }
    }

    /// The square on `file` (0 = a) and `rank` (0 = first rank). Both must be below 8.
    #[inline]
    pub const fn from_file_rank(file: u8, rank: u8) -> Square {
        debug_assert!(file < 8 && rank < 8);
        Square::from_index(((rank & 7) << 3) | (file & 7))
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// File, 0 = a through 7 = h.
    #[inline]
    pub const fn file(self) -> u8 {
        self as u8 & 7
    }

    /// Rank, 0 = first rank through 7 = eighth rank.
    #[inline]
    pub const fn rank(self) -> u8 {
        self as u8 >> 3
    }

    /// The same square seen from the other side of the board (a1 ↔ a8).
    #[inline]
    pub const fn flip_rank(self) -> Square {
        Square::from_index(self as u8 ^ 56)
    }

    /// The same square mirrored left to right (a1 ↔ h1).
    #[inline]
    pub const fn mirror_file(self) -> Square {
        Square::from_index(self as u8 ^ 7)
    }

    /// The bitboard containing only this square.
    #[inline]
    pub const fn bb(self) -> Bitboard {
        Bitboard(1 << self as u8)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = (b'a' + self.file()) as char;
        let rank = (b'1' + self.rank()) as char;
        write!(f, "{file}{rank}")
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ParseSquareError;

impl fmt::Display for ParseSquareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected a square such as e4")
    }
}

impl core::error::Error for ParseSquareError {}

impl FromStr for Square {
    type Err = ParseSquareError;

    fn from_str(s: &str) -> Result<Square, ParseSquareError> {
        match *s.as_bytes() {
            [file @ b'a'..=b'h', rank @ b'1'..=b'8'] => {
                Ok(Square::from_file_rank(file - b'a', rank - b'1'))
            }
            _ => Err(ParseSquareError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_round_trip() {
        for i in 0..64u8 {
            let sq = Square::from_index(i);
            assert_eq!(sq as u8, i);
            assert_eq!(sq.index(), i as usize);
            assert_eq!(Square::try_from_index(i), Some(sq));
            assert_eq!(Square::from_file_rank(sq.file(), sq.rank()), sq);
        }
        assert_eq!(Square::try_from_index(64), None);
        assert_eq!(Square::try_from_index(255), None);
    }

    #[test]
    fn named_squares() {
        assert_eq!(Square::A1.index(), 0);
        assert_eq!(Square::H1.index(), 7);
        assert_eq!(Square::E4.file(), 4);
        assert_eq!(Square::E4.rank(), 3);
        assert_eq!(Square::H8.index(), 63);
    }

    #[test]
    fn flips() {
        assert_eq!(Square::A1.flip_rank(), Square::A8);
        assert_eq!(Square::E2.flip_rank(), Square::E7);
        assert_eq!(Square::A1.mirror_file(), Square::H1);
        assert_eq!(Square::C6.mirror_file(), Square::F6);
        for i in 0..64u8 {
            let sq = Square::from_index(i);
            assert_eq!(sq.flip_rank().flip_rank(), sq);
            assert_eq!(sq.mirror_file().mirror_file(), sq);
        }
    }

    #[test]
    fn text_round_trip() {
        assert_eq!(Square::E4.to_string(), "e4");
        assert_eq!("e4".parse(), Ok(Square::E4));
        for i in 0..64u8 {
            let sq = Square::from_index(i);
            assert_eq!(sq.to_string().parse(), Ok(sq));
        }
        for bad in ["", "e", "e9", "i1", "E4", "e44", " e4"] {
            assert_eq!(bad.parse::<Square>(), Err(ParseSquareError), "{bad:?}");
        }
    }

    #[test]
    fn single_square_bitboard() {
        assert_eq!(Square::A1.bb(), Bitboard(1));
        assert_eq!(Square::H8.bb(), Bitboard(1 << 63));
    }
}
