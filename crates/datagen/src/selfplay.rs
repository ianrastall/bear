//! Deterministic, game-labelled positions from Bear's own play.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::atomic::AtomicBool;

use bear_core::eval::Hce;
use bear_core::movepick::is_noisy;
use bear_core::search::{Shared, ThreadData};
use bear_core::time::Limits;
use bear_core::tt::TranspositionTable;
use bear_core::{Color, PieceType};

use super::{Rng, WORKER_STACK, random_opening};

pub const USAGE: &str =
    "datagen selfplay --out <file> [--games N] [--nodes N] [--seed S] [--workers N]";
pub const FORMAT: &str = "# bear-texel-v1";

struct Options {
    out: String,
    games: usize,
    nodes: u64,
    seed: u64,
    workers: usize,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        out: String::new(),
        games: 400,
        nodes: 2500,
        seed: 1,
        workers: 1,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
        let number = || {
            value
                .parse::<usize>()
                .map_err(|_| format!("invalid {arg}: {value}"))
        };
        match arg.as_str() {
            "--out" => options.out.clone_from(value),
            "--games" => options.games = number()?,
            "--nodes" => {
                options.nodes = value
                    .parse()
                    .map_err(|_| format!("invalid node count: {value}"))?
            }
            "--seed" => {
                options.seed = value
                    .parse()
                    .map_err(|_| format!("invalid seed: {value}"))?
            }
            "--workers" => options.workers = number()?,
            _ => return Err(format!("unknown option {arg}\n{USAGE}")),
        }
    }
    if options.out.is_empty() || options.games == 0 || options.nodes == 0 || options.workers == 0 {
        return Err(USAGE.to_string());
    }
    Ok(options)
}

struct Game {
    index: usize,
    // Twice White's score: 0 for a loss, 1 for a draw, 2 for a win.
    result: u8,
    positions: Vec<String>,
}

fn play(index: usize, options: &Options, shared: Shared<'_>) -> Option<Game> {
    let seed = options
        .seed
        .wrapping_add(index as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        | 1;
    let (mut pos, ci, mut history) = random_opening(&mut Rng(seed), 8)?;
    shared.tt.clear();
    let mut thread = ThreadData::new(Hce::default());
    let limits = Limits {
        nodes: Some(options.nodes),
        ..Limits::default()
    };
    let mut positions = Vec::new();
    let mut streak = 0_i32;
    let decided = |result, positions| {
        Some(Game {
            index,
            result,
            positions,
        })
    };
    for ply in 0..400 {
        thread.set_position(&pos, &ci, &history);
        let search = thread.search(shared, &limits, 0, |_| {});
        let Some(mv) = search.best else {
            let result = if pos.in_check() {
                2 * u8::from(pos.stm() == Color::Black)
            } else {
                1
            };
            return decided(result, positions);
        };
        let reversible = pos.state().halfmove as usize;
        let repeats = history
            .iter()
            .rev()
            .take(reversible)
            .filter(|&&key| key == pos.key())
            .take(2)
            .count();
        let minor = pos.pieces(PieceType::Knight) | pos.pieces(PieceType::Bishop);
        let other = pos.pieces(PieceType::Pawn)
            | pos.pieces(PieceType::Rook)
            | pos.pieces(PieceType::Queen);
        if reversible >= 100 || repeats == 2 || (other.is_empty() && !minor.more_than_one()) {
            return decided(1, positions);
        }

        let white_score = if pos.stm() == Color::White {
            search.score
        } else {
            -search.score
        };
        if white_score.abs() >= 1200 {
            let sign = white_score.signum();
            streak = if streak.signum() == sign {
                streak + sign
            } else {
                sign
            };
            if streak.abs() >= 12 {
                return decided(2 * u8::from(streak > 0), positions);
            }
        } else {
            streak = 0;
        }
        // Sample both sides to move and skip positions that require tactical play.
        if ply >= 4
            && (ply + index).is_multiple_of(3)
            && !pos.in_check()
            && !is_noisy(mv)
            && !mv.is_promotion()
            && white_score.abs() <= 1000
        {
            positions.push(pos.fen(&ci));
        }
        history.push(pos.key());
        pos = pos.make(mv, &ci);
    }
    // Unresolved games carry no reliable result label.
    None
}

pub fn run(args: &[String]) -> Result<(), String> {
    let options = parse(args)?;
    let workers = options.workers.min(options.games);
    let mut games = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for worker in 0..workers {
            let options = &options;
            let handle = std::thread::Builder::new()
                .stack_size(WORKER_STACK)
                .spawn_scoped(scope, move || {
                    let stop = AtomicBool::new(false);
                    let tt = TranspositionTable::new(4);
                    let shared = Shared {
                        stop: &stop,
                        tt: &tt,
                    };
                    (worker..options.games)
                        .step_by(workers)
                        .filter_map(|index| play(index, options, shared))
                        .collect::<Vec<_>>()
                })
                .map_err(|err| format!("cannot start self-play worker: {err}"))?;
            handles.push(handle);
        }
        let mut games = Vec::new();
        for handle in handles {
            games.extend(
                handle
                    .join()
                    .map_err(|_| "self-play worker failed".to_string())?,
            );
        }
        Ok::<_, String>(games)
    })?;
    games.sort_unstable_by_key(|game| game.index);
    let positions = games.iter().map(|game| game.positions.len()).sum::<usize>();
    if positions == 0 {
        return Err("self-play produced no labelled positions".to_string());
    }
    let write = || -> std::io::Result<()> {
        let mut output = BufWriter::new(File::create(&options.out)?);
        writeln!(output, "{FORMAT}")?;
        for game in &games {
            for fen in &game.positions {
                writeln!(output, "{}\t{}\t{}", game.index, game.result, fen)?;
            }
        }
        output.flush()
    };
    write().map_err(|err| format!("cannot write {}: {err}", options.out))?;
    println!(
        "wrote {positions} positions from {} completed games to {}",
        games.len(),
        options.out
    );
    Ok(())
}
