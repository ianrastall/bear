//! Parsing of UCI input lines into commands.

/// A command the engine understands. More variants arrive with the features they drive.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Command {
    Uci,
    IsReady,
    UciNewGame,
    Quit,
}

/// Parses one input line.
///
/// The UCI specification says unknown leading tokens are skipped and the rest of the
/// line is tried as a command, so `"joho isready"` parses as `isready`. A line with no
/// known command yields `None` and is ignored by the caller.
pub fn parse(line: &str) -> Option<Command> {
    line.split_whitespace().find_map(|token| match token {
        "uci" => Some(Command::Uci),
        "isready" => Some(Command::IsReady),
        "ucinewgame" => Some(Command::UciNewGame),
        "quit" => Some(Command::Quit),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_commands() {
        assert_eq!(parse("uci"), Some(Command::Uci));
        assert_eq!(parse("isready"), Some(Command::IsReady));
        assert_eq!(parse("ucinewgame"), Some(Command::UciNewGame));
        assert_eq!(parse("quit"), Some(Command::Quit));
    }

    #[test]
    fn whitespace_is_ignored() {
        assert_eq!(parse("  uci  "), Some(Command::Uci));
        assert_eq!(parse("\tisready\r"), Some(Command::IsReady));
    }

    #[test]
    fn unknown_leading_tokens_are_skipped() {
        assert_eq!(parse("joho isready"), Some(Command::IsReady));
    }

    #[test]
    fn unknown_lines_yield_nothing() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("   "), None);
        assert_eq!(parse("xyzzy"), None);
        assert_eq!(parse("UCI"), None);
    }
}
