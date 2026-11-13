//! Drives the built `bear` binary over real pipes, the way a GUI does.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

struct Bear {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Bear {
    fn start() -> Bear {
        let mut child = Command::new(env!("CARGO_BIN_EXE_bear"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("bear should start");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout: ChildStdout = child.stdout.take().expect("stdout is piped");

        // Read on a helper thread so that a missing reply fails the test instead of
        // hanging it.
        let (sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Bear {
            child,
            stdin,
            lines,
        }
    }

    fn send(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").expect("bear should accept input");
        self.stdin.flush().expect("bear should accept input");
    }

    /// Reads lines until one starts with `prefix`, returning everything read.
    fn read_until(&mut self, prefix: &str, timeout: Duration) -> Vec<String> {
        let deadline = Instant::now() + timeout;
        let mut seen = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    let done = line.starts_with(prefix);
                    seen.push(line);
                    if done {
                        return seen;
                    }
                }
                Err(_) => {
                    panic!("no line starting with {prefix:?} within {timeout:?}; got {seen:?}")
                }
            }
        }
    }

    fn quit(mut self) {
        self.send("quit");
        let status = self.child.wait().expect("bear should exit");
        assert!(status.success(), "exit status: {status}");
    }
}

const WAIT: Duration = Duration::from_secs(20);

#[test]
fn handshake() {
    let mut bear = Bear::start();
    bear.send("uci");
    let lines = bear.read_until("uciok", WAIT);
    assert!(lines[0].starts_with("id name Bear "), "{lines:?}");
    assert_eq!(lines[1], "id author Ian Rastall");
    bear.send("isready");
    bear.read_until("readyok", WAIT);
    bear.quit();
}

#[test]
fn fixed_depth_search_is_complete_and_repeatable() {
    let mut answers = Vec::new();
    for _ in 0..2 {
        let mut bear = Bear::start();
        bear.send("position startpos moves e2e4 e7e5");
        bear.send("go depth 4");
        let lines = bear.read_until("bestmove", WAIT);
        let depths = lines
            .iter()
            .filter(|l| l.starts_with("info depth "))
            .count();
        assert_eq!(depths, 4, "{lines:?}");
        // Drop the time-dependent fields before comparing the two runs.
        let last_info = lines[lines.len() - 2].clone();
        let nodes = last_info.split(" nps ").next().unwrap().to_string();
        answers.push((nodes, lines.last().unwrap().clone()));
        bear.quit();
    }
    assert_eq!(answers[0], answers[1]);
}

#[test]
fn infinite_search_waits_for_stop_and_answers_isready_meanwhile() {
    let mut bear = Bear::start();
    bear.send("position startpos");
    bear.send("go infinite");
    // Let it get going, then check that it stays responsive without answering yet.
    bear.read_until("info depth 3", WAIT);
    bear.send("isready");
    let lines = bear.read_until("readyok", WAIT);
    assert!(
        lines.iter().all(|l| !l.starts_with("bestmove")),
        "{lines:?}"
    );

    bear.send("stop");
    let lines = bear.read_until("bestmove", WAIT);
    assert!(lines.last().unwrap().len() >= "bestmove e2e4".len());

    // A second search works after the first was stopped.
    bear.send("go depth 2");
    bear.read_until("bestmove", WAIT);
    bear.quit();
}

#[test]
fn infinite_search_of_a_finished_game_still_waits_for_stop() {
    let mut bear = Bear::start();
    // Stalemate: there is nothing to search.
    bear.send("position fen 7k/5Q2/6K1/8/8/8/8/8 b - - 0 1");
    bear.send("go infinite");
    bear.send("isready");
    let lines = bear.read_until("readyok", WAIT);
    assert!(
        lines.iter().all(|l| !l.starts_with("bestmove")),
        "{lines:?}"
    );
    bear.send("stop");
    let lines = bear.read_until("bestmove", WAIT);
    assert_eq!(lines.last().unwrap(), "bestmove 0000");
    bear.quit();
}

#[test]
fn movetime_is_respected() {
    let mut bear = Bear::start();
    bear.send("position startpos");
    let start = Instant::now();
    bear.send("go movetime 300");
    bear.read_until("bestmove", WAIT);
    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(600), "{elapsed:?}");
    bear.quit();
}

#[test]
fn command_line_arguments_run_one_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_bear"))
        .args(["bench", "2"])
        .stdin(Stdio::null())
        .output()
        .expect("bear should run");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let words: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(words.len(), 4, "{text}");
    assert!(words[0].parse::<u64>().unwrap() > 0);
    assert_eq!((words[1], words[3]), ("nodes", "nps"));
}
