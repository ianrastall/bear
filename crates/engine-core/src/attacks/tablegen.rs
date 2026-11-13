//! Slow, direct attack generation and the layout of the slider lookup table.
//!
//! This file has no dependencies and works on raw `u64` and `u8` values because it is
//! compiled three times: as a module of this crate, by this crate's `build.rs` (which
//! fills the lookup tables) and by `xtask` (which searches for magic numbers).

#![allow(dead_code)]

pub const ROOK_TABLE_SIZE: usize = 102_400;
pub const BISHOP_TABLE_SIZE: usize = 5_248;
pub const SLIDER_TABLE_SIZE: usize = ROOK_TABLE_SIZE + BISHOP_TABLE_SIZE;

const FILE_A: u64 = 0x0101_0101_0101_0101;
const FILE_H: u64 = 0x8080_8080_8080_8080;
const RANK_1: u64 = 0x0000_0000_0000_00FF;
const RANK_8: u64 = 0xFF00_0000_0000_0000;

const ROOK_DIRS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Squares reached from `sq` along `dirs`, stopping at and including the first blocker.
const fn slide(sq: u8, occ: u64, dirs: &[(i8, i8); 4]) -> u64 {
    let mut attacks = 0u64;
    let mut d = 0;
    while d < 4 {
        let (df, dr) = dirs[d];
        let mut file = (sq & 7) as i8 + df;
        let mut rank = (sq >> 3) as i8 + dr;
        while file >= 0 && file < 8 && rank >= 0 && rank < 8 {
            let bit = 1u64 << (rank * 8 + file);
            attacks |= bit;
            if occ & bit != 0 {
                break;
            }
            file += df;
            rank += dr;
        }
        d += 1;
    }
    attacks
}

pub const fn rook_attacks(sq: u8, occ: u64) -> u64 {
    slide(sq, occ, &ROOK_DIRS)
}

pub const fn bishop_attacks(sq: u8, occ: u64) -> u64 {
    slide(sq, occ, &BISHOP_DIRS)
}

/// Board edges that never affect a slider on `sq`: a blocker on the last square of a
/// ray changes nothing, so those squares are left out of the relevant occupancy.
const fn irrelevant_edges(sq: u8) -> u64 {
    let rank = RANK_1 << (8 * (sq >> 3));
    let file = FILE_A << (sq & 7);
    ((RANK_1 | RANK_8) & !rank) | ((FILE_A | FILE_H) & !file)
}

/// Squares whose occupancy changes a rook's attacks from `sq`.
pub const fn rook_mask(sq: u8) -> u64 {
    rook_attacks(sq, 0) & !irrelevant_edges(sq)
}

/// Squares whose occupancy changes a bishop's attacks from `sq`.
pub const fn bishop_mask(sq: u8) -> u64 {
    bishop_attacks(sq, 0) & !irrelevant_edges(sq)
}

/// Squares strictly between `a` and `b` when they share a rank, file or diagonal; else empty.
pub const fn between(a: u8, b: u8) -> u64 {
    let (bit_a, bit_b) = (1u64 << a, 1u64 << b);
    if rook_attacks(a, 0) & bit_b != 0 {
        rook_attacks(a, bit_b) & rook_attacks(b, bit_a)
    } else if bishop_attacks(a, 0) & bit_b != 0 {
        bishop_attacks(a, bit_b) & bishop_attacks(b, bit_a)
    } else {
        0
    }
}

/// The whole rank, file or diagonal through `a` and `b`, edge to edge; else empty.
pub const fn line(a: u8, b: u8) -> u64 {
    let (bit_a, bit_b) = (1u64 << a, 1u64 << b);
    if rook_attacks(a, 0) & bit_b != 0 {
        (rook_attacks(a, 0) & rook_attacks(b, 0)) | bit_a | bit_b
    } else if bishop_attacks(a, 0) & bit_b != 0 {
        (bishop_attacks(a, 0) & bishop_attacks(b, 0)) | bit_a | bit_b
    } else {
        0
    }
}

/// Lookup parameters for one slider on one square.
#[derive(Copy, Clone)]
pub struct Magic {
    pub mask: u64,
    pub magic: u64,
    pub offset: u32,
    pub shift: u32,
}

impl Magic {
    /// Position in the slider table of the attack set for occupancy `occ`.
    ///
    /// The result is below `offset + (1 << (64 - shift))`.
    #[inline(always)]
    pub const fn index(&self, occ: u64) -> usize {
        self.offset as usize + ((occ & self.mask).wrapping_mul(self.magic) >> self.shift) as usize
    }
}

/// Per-square lookup parameters, with table ranges laid out consecutively from `base`.
pub const fn magics(numbers: &[u64; 64], rook: bool, base: usize) -> [Magic; 64] {
    let mut out = [Magic {
        mask: 0,
        magic: 0,
        offset: 0,
        shift: 0,
    }; 64];
    let mut offset = base;
    let mut sq = 0;
    while sq < 64 {
        let mask = if rook {
            rook_mask(sq as u8)
        } else {
            bishop_mask(sq as u8)
        };
        let bits = mask.count_ones();
        out[sq] = Magic {
            mask,
            magic: numbers[sq],
            offset: offset as u32,
            shift: 64 - bits,
        };
        offset += 1 << bits;
        sq += 1;
    }
    out
}

/// One past the last table index used by `entries`.
pub const fn table_end(entries: &[Magic; 64]) -> usize {
    let last = &entries[63];
    last.offset as usize + (1usize << (64 - last.shift))
}

/// Writes every attack set for `entries` into `table`.
///
/// Returns the square whose magic number maps two occupancies with different attack
/// sets to the same slot, if there is one. Slider attack sets are never empty, so an
/// untouched slot is recognised by zero.
pub fn fill(table: &mut [u64], entries: &[Magic; 64], rook: bool) -> Result<(), u8> {
    for sq in 0..64u8 {
        let entry = &entries[sq as usize];
        let mut subset = 0u64;
        loop {
            let attacks = if rook {
                rook_attacks(sq, subset)
            } else {
                bishop_attacks(sq, subset)
            };
            let slot = &mut table[entry.index(subset)];
            if *slot != 0 && *slot != attacks {
                return Err(sq);
            }
            *slot = attacks;
            subset = subset.wrapping_sub(entry.mask) & entry.mask;
            if subset == 0 {
                break;
            }
        }
    }
    Ok(())
}
