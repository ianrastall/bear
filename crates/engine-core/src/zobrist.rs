//! Zobrist hashing keys, fixed at compile time.

/// The random numbers XORed together to hash a position.
pub struct Keys {
    /// Indexed by piece, then square.
    pub piece_square: [[u64; 64]; 12],
    /// Indexed by the castling rights value.
    pub castling: [u64; 16],
    /// Indexed by the en passant square's file.
    pub ep_file: [u64; 8],
    /// Present when Black is to move.
    pub side: u64,
}

const fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Keys {
    const fn new() -> Keys {
        let mut state = 0x0BEA_2026_C0DE_5EED;
        let mut keys = Keys {
            piece_square: [[0; 64]; 12],
            castling: [0; 16],
            ep_file: [0; 8],
            side: 0,
        };

        let mut piece = 0;
        while piece < 12 {
            let mut sq = 0;
            while sq < 64 {
                keys.piece_square[piece][sq] = splitmix64(&mut state);
                sq += 1;
            }
            piece += 1;
        }
        let mut i = 0;
        while i < 16 {
            keys.castling[i] = splitmix64(&mut state);
            i += 1;
        }
        let mut i = 0;
        while i < 8 {
            keys.ep_file[i] = splitmix64(&mut state);
            i += 1;
        }
        keys.side = splitmix64(&mut state);
        keys
    }
}

pub static KEYS: Keys = Keys::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_distinct_and_nonzero() {
        let mut all: Vec<u64> = KEYS.piece_square.iter().flatten().copied().collect();
        all.extend(KEYS.castling);
        all.extend(KEYS.ep_file);
        all.push(KEYS.side);
        assert_eq!(all.len(), 12 * 64 + 16 + 8 + 1);
        assert!(all.iter().all(|&k| k != 0));
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 12 * 64 + 16 + 8 + 1);
    }
}
