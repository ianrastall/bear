//! Differential fuzzing: play random games and compare the per-move perft counts of
//! every position with another UCI engine's `go perft` output.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

use bear_core::movegen::{MoveList, gen_legal};
use bear_core::perft::{divide, perft};
use bear_core::{CastlingInfo, Position, START_FEN};

/// Positions random games are started from, chosen to reach castling, en passant,
/// promotions and endgames sooner than the starting position does.
const ROOTS: [&str; 6] = [
    START_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
];

const MAX_PLIES: usize = 200;
const EMIT_DEPTH: u32 = 4;

struct Options {
    oracle: String,
    positions: u64,
    depth: u32,
    seed: u64,
    chess960: bool,
    emit: Option<String>,
    emit_count: u64,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        oracle: String::new(),
        positions: 100_000,
        depth: 3,
        seed: 1,
        chess960: false,
        emit: None,
        emit_count: 0,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        let number = |text: &String| {
            text.parse::<u64>()
                .map_err(|_| format!("bad number {text}"))
        };
        match arg.as_str() {
            "--oracle" => options.oracle = value()?.clone(),
            "--positions" => options.positions = number(value()?)?,
            "--depth" => options.depth = number(value()?)? as u32,
            "--seed" => options.seed = number(value()?)?,
            "--chess960" => options.chess960 = true,
            "--emit" => options.emit = Some(value()?.clone()),
            "--emit-count" => options.emit_count = number(value()?)?,
            other => return Err(format!("unknown option {other}")),
        }
    }
    if options.oracle.is_empty() {
        return Err("fuzz needs --oracle <engine>".to_string());
    }
    if options.depth == 0 {
        return Err("--depth must be at least 1".to_string());
    }
    if options.emit.is_some() != (options.emit_count > 0) {
        return Err("--emit and --emit-count go together".to_string());
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

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A running UCI engine used only for its `go perft` output.
struct Oracle {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Oracle {
    fn start(path: &str, chess960: bool) -> Result<Oracle, String> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start {path}: {e}"))?;
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let mut oracle = Oracle {
            child,
            stdin,
            stdout,
        };

        oracle.send("uci")?;
        oracle.read_until("uciok")?;
        if chess960 {
            oracle.send("setoption name UCI_Chess960 value true")?;
        }
        oracle.send("isready")?;
        oracle.read_until("readyok")?;
        Ok(oracle)
    }

    fn send(&mut self, line: &str) -> Result<(), String> {
        writeln!(self.stdin, "{line}")
            .and_then(|()| self.stdin.flush())
            .map_err(|e| format!("oracle stopped accepting input: {e}"))
    }

    fn read_line(&mut self) -> Result<String, String> {
        let mut line = String::new();
        match self.stdout.read_line(&mut line) {
            Ok(0) => Err("oracle closed its output".to_string()),
            Ok(_) => Ok(line.trim().to_string()),
            Err(e) => Err(format!("cannot read from oracle: {e}")),
        }
    }

    fn read_until(&mut self, expected: &str) -> Result<(), String> {
        while self.read_line()? != expected {}
        Ok(())
    }

    /// Per-move counts and the total for `go perft <depth>` on `fen`.
    fn divide(&mut self, fen: &str, depth: u32) -> Result<(BTreeMap<String, u64>, u64), String> {
        self.send(&format!("position fen {fen}"))?;
        self.send(&format!("go perft {depth}"))?;
        let mut moves = BTreeMap::new();
        loop {
            let line = self.read_line()?;
            if let Some(total) = line.strip_prefix("Nodes searched:") {
                let total = total
                    .trim()
                    .parse()
                    .map_err(|_| format!("bad total: {line}"))?;
                return Ok((moves, total));
            }
            // Move lines look like "e2e4: 20"; anything else is engine chatter.
            if let Some((mv, count)) = line.split_once(": ")
                && (4..=5).contains(&mv.len())
                && mv.bytes().all(|b| b.is_ascii_alphanumeric())
                && let Ok(count) = count.trim().parse()
            {
                moves.insert(mv.to_string(), count);
            }
        }
    }
}

impl Drop for Oracle {
    fn drop(&mut self) {
        let _ = self.send("quit");
        let _ = self.child.wait();
    }
}

/// A random Chess960 starting position: bishops on opposite colours, king between rooks.
fn chess960_start(rng: &mut Rng) -> String {
    let mut back = *b"RNBQKBNR";
    loop {
        for i in (1..8).rev() {
            back.swap(i, rng.below(i + 1));
        }
        let find = |piece: u8| back.iter().position(|&p| p == piece).unwrap();
        let rfind = |piece: u8| back.iter().rposition(|&p| p == piece).unwrap();
        let (king, rook_a, rook_h) = (find(b'K'), find(b'R'), rfind(b'R'));
        if find(b'B') % 2 != rfind(b'B') % 2 && rook_a < king && king < rook_h {
            let white = String::from_utf8(back.to_vec()).unwrap();
            let black = white.to_ascii_lowercase();
            let (a, h) = ((b'A' + rook_a as u8) as char, (b'A' + rook_h as u8) as char);
            return format!(
                "{black}/pppppppp/8/8/8/8/PPPPPPPP/{white} w {h}{a}{}{} - 0 1",
                h.to_ascii_lowercase(),
                a.to_ascii_lowercase()
            );
        }
    }
}

fn our_divide(
    pos: &Position,
    ci: &CastlingInfo,
    depth: u32,
    chess960: bool,
) -> (BTreeMap<String, u64>, u64) {
    let moves: BTreeMap<String, u64> = divide(pos, ci, depth)
        .into_iter()
        .map(|(mv, n)| (mv.uci(chess960).to_string(), n))
        .collect();
    let total = moves.values().sum();
    (moves, total)
}

fn describe_mismatch(
    fen: &str,
    ours: &BTreeMap<String, u64>,
    theirs: &BTreeMap<String, u64>,
) -> String {
    let mut out = format!("mismatch at: {fen}\n");
    for (mv, n) in ours {
        match theirs.get(mv) {
            None => writeln!(out, "  {mv}: only Bear generates it").unwrap(),
            Some(m) if m != n => writeln!(out, "  {mv}: Bear {n}, oracle {m}").unwrap(),
            Some(_) => {}
        }
    }
    for mv in theirs.keys().filter(|mv| !ours.contains_key(*mv)) {
        writeln!(out, "  {mv}: only the oracle generates it").unwrap();
    }
    out
}

pub fn run(args: &[String]) -> Result<(), String> {
    let options = parse_options(args)?;
    let mut oracle = Oracle::start(&options.oracle, options.chess960)?;
    let mut rng = Rng(options.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);

    let emit_every = options
        .positions
        .checked_div(options.emit_count)
        .unwrap_or(0)
        .max(1);
    let mut emitted = String::new();
    let mut emitted_count = 0u64;

    let start = Instant::now();
    let mut done = 0u64;
    let mut games = 0u64;

    'games: loop {
        let root = if options.chess960 {
            chess960_start(&mut rng)
        } else {
            ROOTS[rng.below(ROOTS.len())].to_string()
        };
        let (mut pos, ci) = Position::from_fen(&root).map_err(|e| format!("{root}: {e}"))?;
        games += 1;

        for _ in 0..MAX_PLIES {
            if done >= options.positions {
                break 'games;
            }
            let fen = pos.fen(&ci);
            let (ours, our_total) = our_divide(&pos, &ci, options.depth, options.chess960);
            let (theirs, their_total) = oracle.divide(&fen, options.depth)?;
            if ours != theirs || our_total != their_total {
                return Err(describe_mismatch(&fen, &ours, &theirs));
            }
            done += 1;

            if emitted_count < options.emit_count && done.is_multiple_of(emit_every) {
                let mut line = fen.clone();
                for depth in 1..=EMIT_DEPTH {
                    let count = perft(&pos, &ci, depth);
                    let (_, oracle_count) = oracle.divide(&fen, depth)?;
                    if count != oracle_count {
                        return Err(format!(
                            "mismatch at depth {depth}: Bear {count}, oracle {oracle_count}: {fen}"
                        ));
                    }
                    write!(line, " ;D{depth} {count}").unwrap();
                }
                emitted.push_str(&line);
                emitted.push('\n');
                emitted_count += 1;
            }
            if done.is_multiple_of(50_000) {
                eprintln!(
                    "{done} positions agree ({:.0} s)",
                    start.elapsed().as_secs_f64()
                );
            }

            let mut legal = MoveList::new();
            gen_legal(&pos, &ci, &mut legal);
            if legal.is_empty() || pos.state().halfmove >= 100 {
                break;
            }
            let pick = legal.as_slice()[rng.below(legal.len())].mv;
            pos = pos.make(pick, &ci);
        }
    }

    if let Some(path) = &options.emit {
        std::fs::write(path, &emitted).map_err(|e| format!("cannot write {path}: {e}"))?;
        println!("wrote {emitted_count} positions to {path}");
    }
    println!(
        "{done} positions from {games} games agree with the oracle at depth {} ({:.1} s)",
        options.depth,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
