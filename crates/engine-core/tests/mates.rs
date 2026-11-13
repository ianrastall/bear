//! Forced mates that the search must find within a fixed number of nodes.
//!
//! A search that prunes reaches great nominal depths on few nodes when one side is
//! winning, so depth says little about the work done; the node count does. What the
//! search reports must be a mate, and never a shorter one than exists.

use std::sync::atomic::{AtomicBool, Ordering};

use bear_core::Position;
use bear_core::eval::Hce;
use bear_core::search::{Shared, ThreadData, mate_in};
use bear_core::time::Limits;
use bear_core::tt::TranspositionTable;

const SUITE: &str = include_str!("mates.epd");

/// Nodes allowed for each position. `BEAR_MATES_NODES` overrides it, and
/// `BEAR_MATES_VERBOSE` prints what each position needed, to find out how a change to
/// the search affects the suite.
const NODE_BUDGET: u64 = 5_000_000;

/// Searches every position of the suite with a mate in at most `longest` moves until
/// a mate is reported or the budget runs out, and returns how many it tried.
fn solve(longest: i32) -> usize {
    let budget = std::env::var("BEAR_MATES_NODES")
        .ok()
        .and_then(|nodes| nodes.parse().ok())
        .unwrap_or(NODE_BUDGET);
    let verbose = std::env::var_os("BEAR_MATES_VERBOSE").is_some();
    let limits = Limits {
        nodes: Some(budget),
        ..Limits::default()
    };
    let stop = AtomicBool::new(false);
    let tt = TranspositionTable::new(16);
    let shared = Shared {
        stop: &stop,
        tt: &tt,
    };

    let mut tried = 0;
    let mut failures = Vec::new();
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
        stop.store(false, Ordering::Relaxed);

        // The first iteration that reports a mate ends the search.
        let mut found = None;
        td.search(shared, &limits, 0, |report| {
            if let Some(mate) = mate_in(report.score) {
                found = Some((mate, report.depth, report.nodes));
                stop.store(true, Ordering::Relaxed);
            }
        });
        if verbose {
            println!("{found:?}: {line}");
        }
        // A mate, and not a shorter one than exists.
        if !found.is_some_and(|(mate, _, _)| mate >= moves) {
            failures.push(format!("{line}: found {found:?} within {budget} nodes"));
        }
        tried += 1;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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
