//! Forced mates that the search must find at the shallowest depth that can see them.

use std::sync::atomic::AtomicBool;

use bear_core::Position;
use bear_core::eval::Hce;
use bear_core::search::{Shared, ThreadData, mate_in};
use bear_core::time::Limits;
use bear_core::tt::TranspositionTable;

const SUITE: &str = include_str!("mates.epd");

/// Searches every position of the suite with a mate in at most `longest` moves to
/// depth 2k - 1, where the mating move is the last ply, and returns how many it tried.
fn solve(longest: i32) -> usize {
    let stop = AtomicBool::new(false);
    let tt = TranspositionTable::new(16);
    let shared = Shared {
        stop: &stop,
        tt: &tt,
    };
    let mut tried = 0;
    for line in SUITE.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (fen, rest) = line.split_once(';').expect("position; mate in k");
        let moves: i32 = rest
            .trim()
            .strip_prefix("mate in ")
            .and_then(|k| k.parse().ok())
            .unwrap_or_else(|| panic!("bad line: {line}"));
        if moves > longest {
            continue;
        }
        let (pos, ci) = Position::from_fen(fen).unwrap_or_else(|e| panic!("{fen}: {e}"));
        let mut td = ThreadData::new(Hce);
        td.set_position(&pos, &ci, &[]);
        tt.clear();
        let outcome = td.search(shared, &Limits::depth(2 * moves - 1), 0, |_| {});
        assert_eq!(mate_in(outcome.score), Some(moves), "{line}");
        tried += 1;
    }
    tried
}

/// The search recurses deeply; give it the stack the engine gives it.
fn on_big_stack(task: fn() -> usize) -> usize {
    std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(task)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
#[cfg_attr(miri, ignore)]
fn mates_in_two() {
    assert_eq!(on_big_stack(|| solve(2)), 12);
}

/// The longer mates take minutes without optimisation, so CI runs them with
/// `cargo test --release`.
#[test]
#[cfg_attr(any(miri, debug_assertions), ignore)]
fn every_mate() {
    assert_eq!(on_big_stack(|| solve(4)), 36);
}
