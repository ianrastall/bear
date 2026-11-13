//! A set of squares stored as one bit per square.

use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use crate::Square;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const ALL: Bitboard = Bitboard(u64::MAX);
    pub const FILE_A: Bitboard = Bitboard(0x0101_0101_0101_0101);
    pub const FILE_H: Bitboard = Bitboard(0x8080_8080_8080_8080);
    pub const RANK_1: Bitboard = Bitboard(0x0000_0000_0000_00FF);
    pub const RANK_8: Bitboard = Bitboard(0xFF00_0000_0000_0000);

    /// All squares on `file` (0 = a). `file` must be below 8.
    #[inline]
    pub const fn file(file: u8) -> Bitboard {
        debug_assert!(file < 8);
        Bitboard(Self::FILE_A.0 << (file & 7))
    }

    /// All squares on `rank` (0 = first rank). `rank` must be below 8.
    #[inline]
    pub const fn rank(rank: u8) -> Bitboard {
        debug_assert!(rank < 8);
        Bitboard(Self::RANK_1.0 << ((rank & 7) * 8))
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn any(self) -> bool {
        self.0 != 0
    }

    /// Number of squares in the set.
    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether the set holds two or more squares.
    #[inline]
    pub const fn more_than_one(self) -> bool {
        self.0 & self.0.wrapping_sub(1) != 0
    }

    #[inline]
    pub const fn contains(self, sq: Square) -> bool {
        self.0 & sq.bb().0 != 0
    }

    #[inline]
    pub const fn with(self, sq: Square) -> Bitboard {
        Bitboard(self.0 | sq.bb().0)
    }

    #[inline]
    pub const fn without(self, sq: Square) -> Bitboard {
        Bitboard(self.0 & !sq.bb().0)
    }

    /// The lowest-indexed square in the set, if any.
    #[inline]
    pub const fn lsb(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::from_index(self.0.trailing_zeros() as u8))
        }
    }

    /// Removes and returns the lowest-indexed square in the set, if any.
    #[inline]
    pub const fn pop_lsb(&mut self) -> Option<Square> {
        let sq = self.lsb();
        self.0 &= self.0.wrapping_sub(1);
        sq
    }
}

impl From<Square> for Bitboard {
    #[inline]
    fn from(sq: Square) -> Bitboard {
        sq.bb()
    }
}

impl BitAnd for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}

impl BitXor for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl Not for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}

impl BitAndAssign for Bitboard {
    #[inline]
    fn bitand_assign(&mut self, rhs: Bitboard) {
        self.0 &= rhs.0;
    }
}

impl BitOrAssign for Bitboard {
    #[inline]
    fn bitor_assign(&mut self, rhs: Bitboard) {
        self.0 |= rhs.0;
    }
}

impl BitXorAssign for Bitboard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Bitboard) {
        self.0 ^= rhs.0;
    }
}

/// Iterates over the squares of a bitboard from a1 upward.
#[derive(Clone)]
pub struct Squares(Bitboard);

impl Iterator for Squares {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Square> {
        self.0.pop_lsb()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.count() as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for Squares {}

impl IntoIterator for Bitboard {
    type Item = Square;
    type IntoIter = Squares;

    #[inline]
    fn into_iter(self) -> Squares {
        Squares(self)
    }
}

/// Prints the board with rank 8 at the top, `X` for set squares and `.` for clear ones.
impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Bitboard({:#018x})", self.0)?;
        for rank in (0..8u8).rev() {
            for file in 0..8u8 {
                let set = self.contains(Square::from_file_rank(file, rank));
                f.write_str(if set { "X" } else { "." })?;
            }
            if rank > 0 {
                f.write_str("\n")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_and_ranks() {
        assert_eq!(Bitboard::file(0), Bitboard::FILE_A);
        assert_eq!(Bitboard::file(7), Bitboard::FILE_H);
        assert_eq!(Bitboard::rank(0), Bitboard::RANK_1);
        assert_eq!(Bitboard::rank(7), Bitboard::RANK_8);
        for i in 0..8u8 {
            assert_eq!(Bitboard::file(i).count(), 8);
            assert_eq!(Bitboard::rank(i).count(), 8);
            for sq in Bitboard::file(i) {
                assert_eq!(sq.file(), i);
            }
            for sq in Bitboard::rank(i) {
                assert_eq!(sq.rank(), i);
            }
        }
        assert_eq!(Bitboard::file(4) & Bitboard::rank(3), Square::E4.bb());
    }

    #[test]
    fn set_operations() {
        let b = Bitboard::EMPTY.with(Square::C3).with(Square::F6);
        assert!(b.contains(Square::C3));
        assert!(b.contains(Square::F6));
        assert!(!b.contains(Square::C4));
        assert_eq!(b.count(), 2);
        assert!(b.more_than_one());
        assert!(!b.without(Square::C3).more_than_one());
        assert!(!Bitboard::EMPTY.more_than_one());
        assert_eq!(b.without(Square::C3).without(Square::F6), Bitboard::EMPTY);
        assert_eq!(!Bitboard::EMPTY, Bitboard::ALL);
        assert_eq!(b ^ b, Bitboard::EMPTY);
        assert_eq!(b | Bitboard::EMPTY, b);
        assert_eq!(b & Bitboard::ALL, b);
        assert!(Bitboard::EMPTY.is_empty());
        assert!(b.any());
    }

    #[test]
    fn assign_operators() {
        let mut b = Bitboard::EMPTY;
        b |= Square::A1.bb();
        b |= Square::H8.bb();
        assert_eq!(b.count(), 2);
        b &= Bitboard::RANK_1;
        assert_eq!(b, Square::A1.bb());
        b ^= Square::A1.bb();
        assert!(b.is_empty());
    }

    #[test]
    fn iteration_is_ascending_and_exact() {
        let b = Square::H8.bb() | Square::A1.bb() | Square::D4.bb();
        let squares: Vec<Square> = b.into_iter().collect();
        assert_eq!(squares, [Square::A1, Square::D4, Square::H8]);
        assert_eq!(b.into_iter().len(), 3);
        assert_eq!(Bitboard::ALL.into_iter().count(), 64);
        assert_eq!(Bitboard::EMPTY.into_iter().next(), None);
    }

    #[test]
    fn lsb_and_pop() {
        let mut b = Square::G7.bb() | Square::B2.bb();
        assert_eq!(b.lsb(), Some(Square::B2));
        assert_eq!(b.pop_lsb(), Some(Square::B2));
        assert_eq!(b.pop_lsb(), Some(Square::G7));
        assert_eq!(b.pop_lsb(), None);
        assert_eq!(b.lsb(), None);
        assert!(b.is_empty());
    }

    #[test]
    fn debug_layout() {
        let text = format!("{:?}", Square::A1.bb() | Square::H8.bb());
        let rows: Vec<&str> = text.lines().skip(1).collect();
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0], ".......X");
        assert_eq!(rows[7], "X.......");
    }
}
