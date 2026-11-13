//! The `bear` binary: reads UCI commands on stdin and answers on stdout.
//!
//! Two threads share the work. The reader (this thread) parses input and acts on
//! `stop`, `quit` and, during a search, `isready` at once. Everything else goes down a
//! channel to the coordinator, which owns the engine and searches on a large stack.
//!
//! Arguments are run as one command followed by `quit`, so `bear bench` prints the
//! benchmark and exits.

#![forbid(unsafe_code)]

mod bench;
mod command;
mod engine;

use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use bear_core::search::Shared;

use command::Command;
use engine::{Engine, Output};

/// The search recurses once per ply and keeps a move list in every frame.
const COORDINATOR_STACK: usize = 16 << 20;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        run(std::io::stdin().lock(), Output::stdout());
    } else {
        run(args.join(" ").as_bytes(), Output::stdout());
    }
}

/// Runs the protocol until `quit` or end of input, then waits for queued work to finish.
fn run(input: impl BufRead, out: Output) {
    let shared = Shared::default();
    let searching = AtomicBool::new(false);
    let (sender, receiver) = mpsc::channel::<Command>();

    std::thread::scope(|scope| {
        let (shared, searching) = (&shared, &searching);
        let coordinator_out = out.clone();
        std::thread::Builder::new()
            .name("coordinator".to_string())
            .stack_size(COORDINATOR_STACK)
            .spawn_scoped(scope, move || {
                let mut engine = Engine::new(coordinator_out);
                for command in receiver {
                    engine.handle(command, shared, searching);
                }
            })
            .expect("the coordinator thread can be started");

        for line in input.lines() {
            let Ok(line) = line else { break };
            let Some(command) = command::parse(&line) else {
                continue;
            };
            let forwarded = match command {
                Command::Stop => {
                    shared.stop.store(true, Ordering::Relaxed);
                    continue;
                }
                Command::Quit => break,
                // The coordinator is busy searching, so answer for it. When it is idle
                // the answer must wait its turn behind earlier commands.
                Command::IsReady if searching.load(Ordering::Relaxed) => {
                    out.line(format_args!("readyok"));
                    continue;
                }
                Command::Go(_) => {
                    // Both flags change here, not in the coordinator, so that a `stop`
                    // or `isready` arriving before the search starts is not lost.
                    shared.stop.store(false, Ordering::Relaxed);
                    searching.store(true, Ordering::Relaxed);
                    command
                }
                other => other,
            };
            if sender.send(forwarded).is_err() {
                break;
            }
        }

        // Quit or end of input: cut short any search, then closing the channel lets
        // the coordinator finish what is queued and exit.
        if searching.load(Ordering::Relaxed) {
            shared.stop.store(true, Ordering::Relaxed);
        }
        drop(sender);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(input: &str) -> String {
        let (out, buffer) = Output::buffer();
        run(input.as_bytes(), out);
        let bytes = buffer.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn handshake() {
        let out = session("uci\nisready\nquit\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], format!("id name Bear {}", engine::VERSION));
        assert_eq!(lines[1], "id author Ian Rastall");
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("option name Move Overhead type spin"))
        );
        assert!(lines.contains(&"option name UCI_Chess960 type check default false"));
        assert_eq!(lines[lines.len() - 2], "uciok");
        assert_eq!(lines[lines.len() - 1], "readyok");
    }

    #[test]
    fn quit_stops_reading() {
        assert_eq!(session("quit\nisready\n"), "");
    }

    #[test]
    fn end_of_input_ends_the_loop() {
        assert_eq!(session("isready"), "readyok\n");
    }

    #[test]
    fn unknown_and_silent_commands_print_nothing() {
        assert_eq!(session("xyzzy\nucinewgame\n\nstop\n"), "");
    }

    #[test]
    fn position_with_moves() {
        let out = session("position startpos moves e2e4 e7e5 g1f3\nd\n");
        assert!(
            out.contains("Fen: rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2"),
            "{out}"
        );
        assert!(out.contains("8  r n b q k b n r"), "{out}");
    }

    #[test]
    fn castling_is_accepted_in_both_notations() {
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
        for castle in ["e1g1", "e1h1"] {
            let out = session(&format!("position fen {fen} moves {castle}\nd\n"));
            assert!(
                out.contains("Fen: r3k2r/8/8/8/8/8/8/R4RK1 b kq - 1 1"),
                "{castle}: {out}"
            );
        }
    }

    #[test]
    fn illegal_move_and_bad_fen_are_reported() {
        let out = session("position startpos moves e2e5 e7e5\nd\n");
        assert!(out.contains("info string illegal move e2e5"), "{out}");
        assert!(out.contains("Fen: rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"));

        let out = session("position startpos moves e2e4\nposition fen 8/8/8/8 w - - 0 1\nd\n");
        assert!(out.contains("info string invalid FEN"), "{out}");
        // The earlier position is kept.
        assert!(out.contains("Fen: rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"));
    }

    #[test]
    fn go_answers_with_a_best_move() {
        let out = session("position startpos\ngo depth 3\n");
        let last = out.lines().last().unwrap();
        assert!(last.starts_with("bestmove "), "{out}");
        assert!(out.contains("info depth 1 "), "{out}");
    }

    #[test]
    fn go_with_no_legal_move() {
        let out = session("position fen 7k/5Q2/6K1/8/8/8/8/8 b - - 0 1\ngo depth 2\n");
        assert_eq!(out.lines().last(), Some("bestmove 0000"));
    }

    #[test]
    fn perft_command() {
        let out = session("position startpos\nperft 3\n");
        assert!(out.contains("e2e4: 600"), "{out}");
        assert!(out.contains("Nodes searched: 8902"), "{out}");
    }

    #[test]
    fn chess960_option_changes_castling_notation() {
        let fen = "4k3/8/8/8/8/8/8/4K2R w K - 0 1";
        let standard = session(&format!("position fen {fen}\nperft 1\n"));
        assert!(
            standard.contains("e1g1: 1") && !standard.contains("e1h1: 1"),
            "{standard}"
        );
        let chess960 = session(&format!(
            "setoption name UCI_Chess960 value true\nposition fen {fen}\nperft 1\n"
        ));
        assert!(
            chess960.contains("e1h1: 1") && !chess960.contains("e1g1: 1"),
            "{chess960}"
        );
    }

    #[test]
    fn options() {
        assert_eq!(session("setoption name Move Overhead value 50\n"), "");
        assert!(session("setoption name Hash value 16\n").contains("info string unknown option"));
        assert!(session("setoption name Move Overhead value lots\n").contains("needs a number"));
    }

    #[test]
    fn eval_command() {
        let out = session("eval\n");
        assert_eq!(out, "Static evaluation: 10 cp for the side to move\n");
    }

    #[test]
    fn bench_runs_to_completion_before_quitting() {
        let first = session("bench 2\n");
        let second = session("bench 2\nquit\n");
        let nodes = |text: &str| {
            text.split_whitespace()
                .next()
                .unwrap()
                .parse::<u64>()
                .unwrap()
        };
        assert!(first.trim_end().ends_with("nps"), "{first}");
        assert_eq!(nodes(&first), nodes(&second));
        assert!(nodes(&first) > 0);
    }
}
