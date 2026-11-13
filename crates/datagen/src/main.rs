//! Generates data from Bear's own play. For now that is opening positions for test
//! matches; self-play training data is added with the neural network.

#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fmt::Write as _;
use std::process::ExitCode;

use bear_core::eval::Hce;
use bear_core::movegen::{MoveList, gen_legal};
use bear_core::search::{Shared, ThreadData};
use bear_core::time::Limits;
use bear_core::{CastlingInfo, Position};

const USAGE: &str = "\
usage:
  datagen openings --out <file> [--count N] [--plies P] [--seed S] [--depth D] [--max-cp C]

Writes N distinct positions (default 2000), one FEN per line. Each is reached by P
random legal moves from the starting position (default 8) and kept only if neither
side is in check and a depth-D search (default 5) scores it within C centipawns of
level (default 60).";

/// The search keeps a move list in every stack frame and quiescence can run deep.
const WORKER_STACK: usize = 16 << 20;

struct Options {
    out: String,
    count: usize,
    plies: usize,
    seed: u64,
    depth: i32,
    max_cp: i32,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        out: String::new(),
        count: 2000,
        plies: 8,
        seed: 1,
        depth: 5,
        max_cp: 60,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        let number = |text: &String| {
            text.parse::<u64>()
                .map_err(|_| format!("bad number {text}"))
        };
        match arg.as_str() {
            "--out" => options.out = value()?.clone(),
            "--count" => options.count = number(value()?)? as usize,
            "--plies" => options.plies = number(value()?)? as usize,
            "--seed" => options.seed = number(value()?)?,
            "--depth" => options.depth = number(value()?)?.clamp(1, 64) as i32,
            "--max-cp" => options.max_cp = number(value()?)?.min(10_000) as i32,
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    if options.out.is_empty() {
        return Err(format!("openings needs --out <file>\n\n{USAGE}"));
    }
    Ok(options)
}

/// xorshift64*.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// Plays `plies` random legal moves from the start. `None` if the game ends first.
fn random_opening(rng: &mut Rng, plies: usize) -> Option<(Position, CastlingInfo, Vec<u64>)> {
    let (mut pos, ci) = Position::startpos();
    let mut history = Vec::with_capacity(plies);
    for _ in 0..plies {
        let mut legal = MoveList::new();
        gen_legal(&pos, &ci, &mut legal);
        if legal.is_empty() {
            return None;
        }
        let pick = legal.as_slice()[(rng.next() % legal.len() as u64) as usize].mv;
        history.push(pos.key());
        pos = pos.make(pick, &ci);
    }
    Some((pos, ci, history))
}

fn openings(options: &Options) -> Result<(), String> {
    let mut rng = Rng(options.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let shared = Shared::default();
    let limits = Limits::depth(options.depth);
    let mut thread = ThreadData::new(Hce);

    let mut seen = HashSet::new();
    let mut text = String::new();
    let mut tried = 0u64;
    while seen.len() < options.count {
        tried += 1;
        if tried > 1000 * options.count as u64 + 1000 {
            return Err(format!(
                "only {} positions found in {tried} attempts",
                seen.len()
            ));
        }
        let Some((pos, ci, history)) = random_opening(&mut rng, options.plies) else {
            continue;
        };
        if pos.in_check() || seen.contains(&pos.key()) {
            continue;
        }
        thread.set_position(&pos, &ci, &history);
        let outcome = thread.search(&shared, &limits, 0, |_| {});
        if outcome.best.is_none() || outcome.score.abs() > options.max_cp {
            continue;
        }
        seen.insert(pos.key());
        writeln!(text, "{}", pos.fen(&ci)).unwrap();
    }

    std::fs::write(&options.out, text).map_err(|e| format!("cannot write {}: {e}", options.out))?;
    println!(
        "wrote {} positions to {} ({tried} tried)",
        options.count, options.out
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("openings") => parse_options(&args[1..]).and_then(|options| {
            std::thread::Builder::new()
                .stack_size(WORKER_STACK)
                .spawn(move || openings(&options))
                .map_err(|e| format!("cannot start the worker thread: {e}"))?
                .join()
                .unwrap_or_else(|_| Err("the worker thread panicked".to_string()))
        }),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
