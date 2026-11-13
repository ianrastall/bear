//! Perft: counting the leaf positions of the legal move tree to a fixed depth.
//!
//! The counts for well-known positions are published, so agreeing with them checks
//! move generation, legality and make together.

use crate::castling::CastlingInfo;
use crate::movegen::{MoveList, gen_all};
use crate::{Move, Position};

/// Number of positions reachable in exactly `depth` plies.
///
/// At the last ply the legal moves are counted without being made.
pub fn perft(pos: &Position, ci: &CastlingInfo, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut list = MoveList::new();
    gen_all(pos, ci, &mut list);
    let mut nodes = 0;
    for mv in list.iter() {
        if !pos.is_legal(mv, ci) {
            continue;
        }
        nodes += if depth == 1 {
            1
        } else {
            perft(&pos.make(mv, ci), ci, depth - 1)
        };
    }
    nodes
}

/// The perft count below each legal move. `depth` counts the move itself and must be
/// at least 1.
pub fn divide(pos: &Position, ci: &CastlingInfo, depth: u32) -> Vec<(Move, u64)> {
    assert!(depth >= 1, "divide needs a depth of at least 1");
    let mut list = MoveList::new();
    gen_all(pos, ci, &mut list);
    list.iter()
        .filter(|&mv| pos.is_legal(mv, ci))
        .map(|mv| (mv, perft(&pos.make(mv, ci), ci, depth - 1)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Published counts by depth, starting at depth 1.
    const CASES: [(&str, &[u64]); 6] = [
        (
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &[20, 400, 8_902, 197_281, 4_865_609],
        ),
        (
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            &[48, 2_039, 97_862, 4_085_603],
        ),
        (
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            &[14, 191, 2_812, 43_238, 674_624],
        ),
        (
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            &[6, 264, 9_467, 422_333],
        ),
        (
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            &[44, 1_486, 62_379, 2_103_487],
        ),
        (
            "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
            &[46, 2_079, 89_890, 3_894_594],
        ),
    ];

    #[test]
    fn published_counts() {
        // Under Miri only the shallow depths are affordable.
        let limit = if cfg!(miri) { 2 } else { usize::MAX };
        for (fen, counts) in CASES {
            let (pos, ci) = Position::from_fen(fen).unwrap();
            for (i, &expected) in counts.iter().enumerate().take(limit) {
                let depth = i as u32 + 1;
                assert_eq!(perft(&pos, &ci, depth), expected, "depth {depth} of {fen}");
            }
        }
    }

    #[test]
    fn depth_zero_is_one_node() {
        let (pos, ci) = Position::startpos();
        assert_eq!(perft(&pos, &ci, 0), 1);
    }

    #[test]
    fn divide_sums_to_perft() {
        let (pos, ci) = Position::from_fen(CASES[1].0).unwrap();
        let parts = divide(&pos, &ci, 2);
        assert_eq!(parts.len(), 48);
        assert_eq!(parts.iter().map(|&(_, n)| n).sum::<u64>(), 2_039);
    }

    #[test]
    fn mirrored_positions_have_equal_counts() {
        // The same position with colours swapped and the board flipped.
        let white = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
        let black = "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1";
        let (a, a_ci) = Position::from_fen(white).unwrap();
        let (b, b_ci) = Position::from_fen(black).unwrap();
        let depth = if cfg!(miri) { 1 } else { 3 };
        assert_eq!(perft(&a, &a_ci, depth), perft(&b, &b_ci, depth));
    }
}
