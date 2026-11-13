//! Move-generation checks for Bear: perft counts, a suite runner and a differential
//! fuzzer that compares against another engine's `go perft` output.

#![forbid(unsafe_code)]

mod fuzz;

use std::process::ExitCode;
use std::time::Instant;

use bear_core::perft::{divide, perft};
use bear_core::{Position, START_FEN};

const USAGE: &str = "\
usage:
  perft <depth> [fen]                 count positions at <depth>
  perft divide <depth> [fen]          the count below each legal move
  perft suite <file> [--max-depth N]  check every line of an EPD file
  perft fuzz --oracle <engine> [--positions N] [--depth D] [--seed S]
             [--chess960] [--emit <file> --emit-count K]
                                      compare random positions with another engine

The FEN defaults to the starting position. Suite lines look like:
  <fen> ;D1 20 ;D2 400 ;D3 8902";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("divide") => run_perft(&args[1..], true),
        Some("suite") => run_suite(&args[1..]),
        Some("fuzz") => fuzz::run(&args[1..]),
        Some("-h" | "--help") | None => Err(USAGE.to_string()),
        Some(_) => run_perft(&args, false),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn nodes_per_second(nodes: u64, seconds: f64) -> u64 {
    if seconds > 0.0 {
        (nodes as f64 / seconds) as u64
    } else {
        0
    }
}

fn run_perft(args: &[String], per_move: bool) -> Result<(), String> {
    let depth: u32 = args
        .first()
        .and_then(|d| d.parse().ok())
        .ok_or_else(|| format!("expected a depth\n\n{USAGE}"))?;
    let fen = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        START_FEN.to_string()
    };
    let (pos, ci) = Position::from_fen(&fen).map_err(|e| format!("bad FEN: {e}"))?;
    // File-letter castling rights mean Chess960, where castling is written king-takes-rook.
    let chess960 = fen.split_whitespace().nth(2).is_some_and(is_shredder);

    let start = Instant::now();
    let nodes = if per_move {
        if depth == 0 {
            return Err("divide needs a depth of at least 1".to_string());
        }
        let mut parts: Vec<(String, u64)> = divide(&pos, &ci, depth)
            .into_iter()
            .map(|(mv, n)| (mv.uci(chess960).to_string(), n))
            .collect();
        parts.sort();
        for (mv, n) in &parts {
            println!("{mv}: {n}");
        }
        println!();
        parts.iter().map(|(_, n)| n).sum()
    } else {
        perft(&pos, &ci, depth)
    };
    let seconds = start.elapsed().as_secs_f64();

    println!("Nodes: {nodes}");
    println!("Time: {:.3} s", seconds);
    println!("NPS: {}", nodes_per_second(nodes, seconds));
    Ok(())
}

/// Whether a castling field uses rook file letters instead of `KQkq`.
fn is_shredder(field: &str) -> bool {
    field
        .chars()
        .any(|c| !matches!(c, 'K' | 'Q' | 'k' | 'q' | '-'))
}

fn run_suite(args: &[String]) -> Result<(), String> {
    let path = args
        .first()
        .ok_or_else(|| format!("expected a suite file\n\n{USAGE}"))?;
    let max_depth: u32 = match args.get(1).map(String::as_str) {
        Some("--max-depth") => args
            .get(2)
            .and_then(|d| d.parse().ok())
            .ok_or("--max-depth needs a number".to_string())?,
        Some(other) => return Err(format!("unknown option {other}")),
        None => u32::MAX,
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;

    let start = Instant::now();
    let mut total_nodes = 0u64;
    let mut checks = 0u32;
    let mut failures = 0u32;

    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split(';');
        let fen = fields.next().unwrap_or_default().trim();
        let (pos, ci) =
            Position::from_fen(fen).map_err(|e| format!("{path}:{}: bad FEN: {e}", number + 1))?;

        for field in fields {
            let mut words = field.split_whitespace();
            let depth = words
                .next()
                .and_then(|d| d.strip_prefix('D'))
                .and_then(|d| d.parse().ok());
            let expected = words.next().and_then(|n| n.parse::<u64>().ok());
            let (Some(depth), Some(expected)): (Option<u32>, _) = (depth, expected) else {
                return Err(format!("{path}:{}: expected ;D<depth> <count>", number + 1));
            };
            if depth > max_depth {
                continue;
            }
            let got = perft(&pos, &ci, depth);
            total_nodes += got;
            checks += 1;
            if got != expected {
                failures += 1;
                println!("FAIL depth {depth}: got {got}, expected {expected}: {fen}");
            }
        }
    }

    let seconds = start.elapsed().as_secs_f64();
    println!(
        "{checks} checks, {failures} failed, {total_nodes} nodes, {seconds:.3} s, {} nps",
        nodes_per_second(total_nodes, seconds)
    );
    if failures == 0 {
        Ok(())
    } else {
        Err(format!("{failures} perft counts differ"))
    }
}
