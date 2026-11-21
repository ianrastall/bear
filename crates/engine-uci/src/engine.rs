//! The coordinator: owns the game state and carries out commands in order.

use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bear_core::eval::Hce;
use bear_core::movegen::find_uci_move;
use bear_core::perft::divide;
use bear_core::search::{Outcome, Report, Shared, ThreadData, mate_in, vote};
use bear_core::time::Limits;
use bear_core::tt::TranspositionTable;
use bear_core::{CastlingInfo, Position};

use crate::bench;
use crate::command::Command;

pub const NAME: &str = "Bear";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHOR: &str = "Ian Rastall";

const DEFAULT_MOVE_OVERHEAD: u64 = 10;
const MAX_MOVE_OVERHEAD: u64 = 5000;
const MAX_THREADS: usize = 256;
/// Helper threads recurse once per ply like the coordinator, so they get as much stack.
const HELPER_STACK: usize = 16 << 20;

/// Where protocol output goes. Clones share one destination, and each line is written
/// and flushed under a lock so that lines from different threads never interleave.
#[derive(Clone)]
pub struct Output(Arc<Mutex<dyn Write + Send>>);

impl Output {
    pub fn stdout() -> Output {
        Output(Arc::new(Mutex::new(std::io::stdout())))
    }

    #[cfg(test)]
    pub fn buffer() -> (Output, Arc<Mutex<Vec<u8>>>) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        (Output(buffer.clone()), buffer)
    }

    /// Writes one line. A closed pipe means the GUI has gone; there is nobody left to
    /// tell, so the error is dropped.
    pub fn line(&self, text: fmt::Arguments) {
        let mut out = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }
}

macro_rules! say {
    ($out:expr, $($arg:tt)*) => {
        $out.line(format_args!($($arg)*))
    };
}

pub struct Engine {
    out: Output,
    pos: Position,
    castling: CastlingInfo,
    /// Keys of every position before the current one, oldest first.
    history: Vec<u64>,
    /// One for each search thread; the first is the coordinator's own.
    threads: Vec<ThreadData<Hce>>,
    tt: TranspositionTable,
    move_overhead: u64,
    chess960: bool,
}

impl Engine {
    pub fn new(out: Output) -> Engine {
        let (pos, castling) = Position::startpos();
        Engine {
            out,
            pos,
            castling,
            history: Vec::new(),
            threads: vec![ThreadData::new(Hce::default())],
            tt: TranspositionTable::new(TranspositionTable::DEFAULT_MB),
            move_overhead: DEFAULT_MOVE_OVERHEAD,
            chess960: false,
        }
    }

    /// Carries out one command. `searching` is cleared once a `go` has been answered.
    pub fn handle(&mut self, command: Command, stop: &AtomicBool, searching: &AtomicBool) {
        match command {
            Command::Uci => self.identify(),
            Command::IsReady => say!(self.out, "readyok"),
            Command::UciNewGame => {
                let count = self.threads.len();
                self.set_threads(count);
                self.tt.clear_parallel(count);
            }
            Command::Position { fen, moves } => self.set_position(fen.as_deref(), &moves),
            Command::Go(limits) => {
                self.go(&limits, stop);
                searching.store(false, Ordering::Relaxed);
            }
            Command::SetOption { name, value } => self.set_option(&name, value.as_deref()),
            Command::D => self.print_board(),
            Command::Eval => self.print_eval(),
            Command::Perft(depth) => self.perft(depth),
            Command::Bench(depth) => {
                let result = bench::run(depth.unwrap_or(bench::DEFAULT_DEPTH).max(1));
                say!(self.out, "{} nodes {} nps", result.nodes, result.nps);
            }
            // Handled by the reader thread before commands reach the coordinator.
            Command::Stop | Command::Quit => {}
        }
    }

    fn identify(&self) {
        say!(self.out, "id name {NAME} {VERSION}");
        say!(self.out, "id author {AUTHOR}");
        say!(
            self.out,
            "option name Hash type spin default {} min 1 max {}",
            TranspositionTable::DEFAULT_MB,
            TranspositionTable::MAX_MB
        );
        say!(
            self.out,
            "option name Threads type spin default 1 min 1 max {MAX_THREADS}"
        );
        say!(
            self.out,
            "option name Move Overhead type spin default {DEFAULT_MOVE_OVERHEAD} min 0 max {MAX_MOVE_OVERHEAD}"
        );
        say!(
            self.out,
            "option name UCI_Chess960 type check default false"
        );
        say!(self.out, "uciok");
    }

    fn set_option(&mut self, name: &str, value: Option<&str>) {
        match (name.to_ascii_lowercase().as_str(), value) {
            ("hash", Some(value)) => match value.parse::<usize>() {
                Ok(megabytes) => {
                    let wanted = megabytes.clamp(1, TranspositionTable::MAX_MB);
                    let got = self.tt.resize(wanted);
                    if got != wanted {
                        say!(
                            self.out,
                            "info string Hash set to {got} MB; {wanted} MB could not be allocated"
                        );
                    }
                }
                Err(_) => say!(self.out, "info string Hash needs a number, got {value}"),
            },
            ("threads", Some(value)) => match value.parse::<usize>() {
                Ok(count) => self.set_threads(count.clamp(1, MAX_THREADS)),
                Err(_) => say!(self.out, "info string Threads needs a number, got {value}"),
            },
            ("move overhead", Some(value)) => match value.parse::<u64>() {
                Ok(ms) => self.move_overhead = ms.min(MAX_MOVE_OVERHEAD),
                Err(_) => say!(
                    self.out,
                    "info string Move Overhead needs a number, got {value}"
                ),
            },
            ("uci_chess960", Some(value)) => self.chess960 = value.eq_ignore_ascii_case("true"),
            _ => say!(self.out, "info string unknown option {name}"),
        }
    }

    /// Replaces every thread's state with a fresh one, `count` threads in all.
    fn set_threads(&mut self, count: usize) {
        self.threads.clear();
        self.threads
            .resize_with(count, || ThreadData::new(Hce::default()));
    }

    /// Sets up `fen` (the starting position when `None`) and plays `moves` on it. On a
    /// bad FEN the previous position is kept; moves after an illegal one are dropped.
    fn set_position(&mut self, fen: Option<&str>, moves: &[String]) {
        let (mut pos, castling) = match fen {
            None => Position::startpos(),
            Some(fen) => match Position::from_fen(fen) {
                Ok(parsed) => parsed,
                Err(error) => {
                    say!(self.out, "info string invalid FEN: {error}");
                    return;
                }
            },
        };
        let mut history = Vec::with_capacity(moves.len());
        for text in moves {
            let Some(mv) = find_uci_move(&pos, &castling, text) else {
                say!(
                    self.out,
                    "info string illegal move {text}; later moves ignored"
                );
                break;
            };
            history.push(pos.key());
            pos = pos.make(mv, &castling);
        }
        self.pos = pos;
        self.castling = castling;
        self.history = history;
    }

    /// Searches on every thread. The main thread keeps the clock and reports; the
    /// helpers search the same position without limits until it has finished, and
    /// then the threads vote on the move to play.
    fn go(&mut self, limits: &Limits, stop: &AtomicBool) {
        let out = &self.out;
        let chess960 = self.chess960;
        let overhead = self.move_overhead;
        let tt = &self.tt;
        let counters = Shared::counters(self.threads.len());
        // Helpers stop when this is set, once the main thread has finished.
        let halt = AtomicBool::new(false);
        let helper_limits = Limits {
            depth: limits.depth,
            ..Limits::default()
        };
        for thread in &mut self.threads {
            thread.set_position(&self.pos, &self.castling, &self.history);
        }
        let (main, helpers) = self
            .threads
            .split_first_mut()
            .expect("there is always a main thread");

        let outcomes: Vec<Outcome> = std::thread::scope(|scope| {
            let (counters, halt, helper_limits) = (&counters, &halt, &helper_limits);
            let running: Vec<_> = (helpers.iter_mut().enumerate())
                .map(|(index, thread)| {
                    let shared = Shared {
                        stop: halt,
                        tt,
                        counters,
                        thread: index + 1,
                    };
                    std::thread::Builder::new()
                        .name(format!("helper {}", index + 1))
                        .stack_size(HELPER_STACK)
                        .spawn_scoped(scope, move || {
                            thread.search(shared, helper_limits, 0, |_| {})
                        })
                        .expect("a helper thread can be started")
                })
                .collect();

            let shared = Shared {
                stop,
                tt,
                counters,
                thread: 0,
            };
            let first = main.search(shared, limits, overhead, |report| {
                print_info(out, report, chess960);
            });

            // An infinite search may not answer until it is told to stop, even if it
            // has nothing left to search.
            if limits.infinite {
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            halt.store(true, Ordering::Relaxed);
            std::iter::once(first)
                .chain(
                    running
                        .into_iter()
                        .map(|helper| helper.join().expect("a helper thread does not panic")),
                )
                .collect()
        });
        let outcome = outcomes[vote(&outcomes)];

        match (outcome.best, outcome.ponder) {
            (Some(best), Some(ponder)) => {
                say!(
                    self.out,
                    "bestmove {} ponder {}",
                    best.uci(chess960),
                    ponder.uci(chess960)
                )
            }
            (Some(best), None) => say!(self.out, "bestmove {}", best.uci(chess960)),
            (None, _) => say!(self.out, "bestmove 0000"),
        }
    }

    fn print_board(&self) {
        say!(self.out, "{}", self.pos);
        say!(self.out, "Fen: {}", self.pos.fen(&self.castling));
        say!(self.out, "Key: {:016x}", self.pos.key());
        let checkers: Vec<String> = self
            .pos
            .checkers()
            .into_iter()
            .map(|sq| sq.to_string())
            .collect();
        say!(self.out, "Checkers: {}", checkers.join(" "));
    }

    fn print_eval(&mut self) {
        let thread = &mut self.threads[0];
        thread.set_position(&self.pos, &self.castling, &self.history);
        say!(
            self.out,
            "Static evaluation: {} cp for the side to move",
            thread.evaluate_root()
        );
    }

    fn perft(&self, depth: u32) {
        let start = Instant::now();
        let mut total = 0;
        if depth == 0 {
            total = 1;
        } else {
            let mut parts: Vec<(String, u64)> = divide(&self.pos, &self.castling, depth)
                .into_iter()
                .map(|(mv, nodes)| (mv.uci(self.chess960).to_string(), nodes))
                .collect();
            parts.sort();
            for (mv, nodes) in parts {
                say!(self.out, "{mv}: {nodes}");
                total += nodes;
            }
        }
        say!(self.out, "");
        say!(self.out, "Nodes searched: {total}");
        say!(self.out, "Time: {} ms", start.elapsed().as_millis());
    }
}

fn print_info(out: &Output, report: &Report, chess960: bool) {
    let score = match mate_in(report.score) {
        Some(moves) => format!("mate {moves}"),
        None => format!("cp {}", report.score),
    };
    let millis = report.elapsed.as_millis();
    let nps = (report.nodes as f64 / report.elapsed.as_secs_f64().max(1e-6)) as u64;
    let pv: Vec<String> = report
        .pv
        .iter()
        .map(|mv| mv.uci(chess960).to_string())
        .collect();
    say!(
        out,
        "info depth {} seldepth {} score {score} nodes {} nps {nps} hashfull {} time {millis} pv {}",
        report.depth,
        report.seldepth,
        report.nodes,
        report.hashfull,
        pv.join(" ")
    );
}
