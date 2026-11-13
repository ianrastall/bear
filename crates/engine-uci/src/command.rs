//! Parsing of UCI input lines into commands.

use bear_core::time::Limits;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    Uci,
    IsReady,
    UciNewGame,
    /// `fen` is `None` for the starting position.
    Position {
        fen: Option<String>,
        moves: Vec<String>,
    },
    Go(Limits),
    Stop,
    Quit,
    SetOption {
        name: String,
        value: Option<String>,
    },
    /// Print the board.
    D,
    /// Print the static evaluation.
    Eval,
    /// Count leaf positions below each legal move.
    Perft(u32),
    /// Search the benchmark positions, optionally to a given depth.
    Bench(Option<i32>),
}

/// Parses one input line.
///
/// The UCI specification says unknown leading tokens are skipped and the rest of the
/// line is tried as a command, so `"joho isready"` parses as `isready`. A line with no
/// known command, or with a command missing what it needs, yields `None` and is ignored.
pub fn parse(line: &str) -> Option<Command> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    for (i, &token) in tokens.iter().enumerate() {
        let rest = &tokens[i + 1..];
        return match token {
            "uci" => Some(Command::Uci),
            "isready" => Some(Command::IsReady),
            "ucinewgame" => Some(Command::UciNewGame),
            "position" => parse_position(rest),
            "go" => Some(Command::Go(parse_go(rest))),
            "stop" => Some(Command::Stop),
            "quit" => Some(Command::Quit),
            "setoption" => parse_setoption(rest),
            "d" => Some(Command::D),
            "eval" => Some(Command::Eval),
            "perft" => rest
                .first()
                .and_then(|d| d.parse().ok())
                .map(Command::Perft),
            "bench" => Some(Command::Bench(rest.first().and_then(|d| d.parse().ok()))),
            _ => continue,
        };
    }
    None
}

fn parse_position(tokens: &[&str]) -> Option<Command> {
    let moves_at = tokens
        .iter()
        .position(|&t| t == "moves")
        .unwrap_or(tokens.len());
    let (setup, moves) = tokens.split_at(moves_at);
    let fen = match setup {
        ["startpos"] => None,
        ["fen", fields @ ..] if !fields.is_empty() => Some(fields.join(" ")),
        _ => return None,
    };
    let moves = moves.iter().skip(1).map(|m| m.to_string()).collect();
    Some(Command::Position { fen, moves })
}

fn parse_go(tokens: &[&str]) -> Limits {
    let mut limits = Limits::default();
    let mut tokens = tokens.iter();
    while let Some(&token) = tokens.next() {
        // Clocks can be reported as negative when a side has overstepped; treat as zero.
        let mut number = || {
            tokens
                .next()
                .and_then(|v| v.parse::<i64>().ok())
                .map(|v| v.max(0) as u64)
        };
        match token {
            "wtime" => limits.time[0] = number(),
            "btime" => limits.time[1] = number(),
            "winc" => limits.inc[0] = number().unwrap_or(0),
            "binc" => limits.inc[1] = number().unwrap_or(0),
            "movestogo" => limits.movestogo = number(),
            "movetime" => limits.movetime = number(),
            "nodes" => limits.nodes = number(),
            "depth" => limits.depth = number().map(|d| d.min(i32::MAX as u64) as i32),
            "infinite" => limits.infinite = true,
            _ => {}
        }
    }
    limits
}

fn parse_setoption(tokens: &[&str]) -> Option<Command> {
    let ["name", rest @ ..] = tokens else {
        return None;
    };
    let value_at = rest
        .iter()
        .position(|&t| t == "value")
        .unwrap_or(rest.len());
    let (name, value) = rest.split_at(value_at);
    if name.is_empty() {
        return None;
    }
    let value = (value.len() > 1).then(|| value[1..].join(" "));
    Some(Command::SetOption {
        name: name.join(" "),
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_commands() {
        assert_eq!(parse("uci"), Some(Command::Uci));
        assert_eq!(parse("isready"), Some(Command::IsReady));
        assert_eq!(parse("ucinewgame"), Some(Command::UciNewGame));
        assert_eq!(parse("stop"), Some(Command::Stop));
        assert_eq!(parse("quit"), Some(Command::Quit));
        assert_eq!(parse("d"), Some(Command::D));
        assert_eq!(parse("eval"), Some(Command::Eval));
    }

    #[test]
    fn whitespace_and_unknown_leading_tokens_are_skipped() {
        assert_eq!(parse("  uci  "), Some(Command::Uci));
        assert_eq!(parse("\tisready\r"), Some(Command::IsReady));
        assert_eq!(parse("joho isready"), Some(Command::IsReady));
    }

    #[test]
    fn unknown_lines_yield_nothing() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("   "), None);
        assert_eq!(parse("xyzzy"), None);
        assert_eq!(parse("UCI"), None);
    }

    #[test]
    fn position() {
        assert_eq!(
            parse("position startpos"),
            Some(Command::Position {
                fen: None,
                moves: vec![]
            })
        );
        assert_eq!(
            parse("position startpos moves e2e4 e7e5"),
            Some(Command::Position {
                fen: None,
                moves: vec!["e2e4".into(), "e7e5".into()]
            })
        );
        assert_eq!(
            parse("position fen 4k3/8/8/8/8/8/8/4K3 w - - 0 1 moves e1e2"),
            Some(Command::Position {
                fen: Some("4k3/8/8/8/8/8/8/4K3 w - - 0 1".into()),
                moves: vec!["e1e2".into()],
            })
        );
        assert_eq!(parse("position"), None);
        assert_eq!(parse("position fen"), None);
        assert_eq!(parse("position elsewhere"), None);
    }

    #[test]
    fn go() {
        assert_eq!(parse("go"), Some(Command::Go(Limits::default())));
        assert_eq!(parse("go depth 7"), Some(Command::Go(Limits::depth(7))));
        assert_eq!(
            parse("go infinite"),
            Some(Command::Go(Limits {
                infinite: true,
                ..Limits::default()
            }))
        );
        assert_eq!(
            parse("go wtime 8000 btime 7500 winc 80 binc 80 movestogo 12"),
            Some(Command::Go(Limits {
                time: [Some(8000), Some(7500)],
                inc: [80, 80],
                movestogo: Some(12),
                ..Limits::default()
            }))
        );
        assert_eq!(
            parse("go movetime 250 nodes 5000"),
            Some(Command::Go(Limits {
                movetime: Some(250),
                nodes: Some(5000),
                ..Limits::default()
            }))
        );
        // A clock that has already run out is zero, not an error.
        assert_eq!(
            parse("go wtime -15 btime 100"),
            Some(Command::Go(Limits {
                time: [Some(0), Some(100)],
                ..Limits::default()
            }))
        );
        // Parameters this version does not act on are skipped.
        assert_eq!(
            parse("go ponder depth 3"),
            Some(Command::Go(Limits::depth(3)))
        );
    }

    #[test]
    fn setoption() {
        assert_eq!(
            parse("setoption name Move Overhead value 30"),
            Some(Command::SetOption {
                name: "Move Overhead".into(),
                value: Some("30".into())
            })
        );
        assert_eq!(
            parse("setoption name UCI_Chess960 value true"),
            Some(Command::SetOption {
                name: "UCI_Chess960".into(),
                value: Some("true".into())
            })
        );
        assert_eq!(
            parse("setoption name Clear Hash"),
            Some(Command::SetOption {
                name: "Clear Hash".into(),
                value: None
            })
        );
        assert_eq!(parse("setoption"), None);
        assert_eq!(parse("setoption name"), None);
    }

    #[test]
    fn debug_commands() {
        assert_eq!(parse("perft 5"), Some(Command::Perft(5)));
        assert_eq!(parse("perft"), None);
        assert_eq!(parse("perft deep"), None);
        assert_eq!(parse("bench"), Some(Command::Bench(None)));
        assert_eq!(parse("bench 6"), Some(Command::Bench(Some(6))));
    }
}
