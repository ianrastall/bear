//! A fixed set of searches whose total node count identifies the engine's behaviour.
//!
//! Any change to search or evaluation changes the count, and a change that claims to
//! alter nothing must leave it alone. The count is the same on every platform.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use bear_core::Position;
use bear_core::eval::Hce;
use bear_core::search::{Shared, ThreadData};
use bear_core::time::Limits;
use bear_core::tt::TranspositionTable;

/// Depth searched when `bench` is given no argument.
pub const DEFAULT_DEPTH: i32 = 6;

/// Openings, middlegames and endgames, with both sides to move.
pub const POSITIONS: [&str; 24] = [
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
    "1r2kb1r/p2p1p2/b1p1pnp1/2qP4/1p2P2Q/1Pn2N1p/P1PB1PPP/1R1BKR2 b k - 1 7",
    "6k1/rp1nr2p/p2p3Q/2pn4/N3PB1P/Pq1P4/R1Pb1PP1/R3N1K1 w - - 3 27",
    "2kr2r1/Pp1p1pp1/3b1nb1/nP1p3P/B1N5/P2P3P/8/q3R1K1 w - - 4 21",
    "3k3r/1pb2p1p/1Pn3p1/3pP2n/7N/r2P4/2Q2RP1/r2B3K w - - 2 22",
    "8/3Pk1rn/1p4n1/7p/B1PBP3/PR5P/5N1b/1Q5K w - - 1 29",
    "2r2N2/6k1/5pp1/1p6/2p1B2p/p1P2n2/PPR3R1/5K2 w - - 0 59",
    "2b1k3/5p1r/1P2N2p/3BnK2/3b3P/4R3/8/3Br3 b - - 1 44",
    "8/1p6/1P3p2/2k5/R1pp1N2/P1b5/5r2/3K4 b - - 18 59",
    "1k6/8/5p2/p3bn1P/P4K2/8/N1P1R3/8 w - - 2 58",
    "4k3/8/np6/1P3K2/8/7P/8/7R b - - 2 71",
    "n1n5/PPPk4/8/8/8/8/4Kppp/5N1N b - - 0 1",
    "8/k7/3p4/p2P1p2/P2P1P2/8/8/K7 w - - 0 1",
    "8/8/1p1k4/p1pP4/P1P1K3/1P6/8/8 w - - 0 1",
    "8/5pk1/6p1/8/3R4/6P1/r4PK1/8 b - - 0 1",
    "8/8/4k3/8/8/4K3/4P3/8 w - - 0 1",
    "4k3/8/8/8/8/8/8/3QK2r w - - 0 1",
];

pub struct BenchResult {
    pub nodes: u64,
    pub nps: u64,
}

/// Searches every position to `depth` and sums the nodes. Each search starts with a
/// fresh thread state and an empty table of the default size.
pub fn run(depth: i32) -> BenchResult {
    let stop = AtomicBool::new(false);
    let tt = TranspositionTable::new(TranspositionTable::DEFAULT_MB);
    let shared = Shared {
        stop: &stop,
        tt: &tt,
    };
    let limits = Limits::depth(depth);
    let start = Instant::now();
    let mut nodes = 0;
    for fen in POSITIONS {
        let (pos, ci) = Position::from_fen(fen).expect("bench positions are valid");
        let mut td = ThreadData::new(Hce);
        td.set_position(&pos, &ci, &[]);
        tt.clear();
        nodes += td.search(shared, &limits, 0, |_| {}).nodes;
    }
    let seconds = start.elapsed().as_secs_f64();
    let nps = if seconds > 0.0 {
        (nodes as f64 / seconds) as u64
    } else {
        0
    };
    BenchResult { nodes, nps }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_are_valid_and_distinct() {
        let mut keys: Vec<u64> = POSITIONS
            .iter()
            .map(|fen| {
                let (pos, ci) = Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen}: {e}"));
                assert_eq!(pos.fen(&ci), *fen);
                pos.key()
            })
            .collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), POSITIONS.len());
    }

    #[test]
    fn node_count_is_reproducible() {
        assert_eq!(run(2).nodes, run(2).nodes);
    }
}
