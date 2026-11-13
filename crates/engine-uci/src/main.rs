//! The `bear` binary: reads UCI commands on stdin and answers on stdout.

#![forbid(unsafe_code)]

mod command;

use std::io::{self, BufRead, Write};

use command::Command;

const NAME: &str = "Bear";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHOR: &str = "Ian Rastall";

fn main() -> io::Result<()> {
    run(io::stdin().lock(), io::stdout().lock())
}

/// Runs the protocol loop until `quit` or end of input.
fn run(input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    for line in input.lines() {
        match command::parse(&line?) {
            Some(Command::Uci) => {
                writeln!(output, "id name {NAME} {VERSION}")?;
                writeln!(output, "id author {AUTHOR}")?;
                writeln!(output, "uciok")?;
            }
            Some(Command::IsReady) => writeln!(output, "readyok")?,
            // Nothing to reset until the engine holds game state.
            Some(Command::UciNewGame) => {}
            Some(Command::Quit) => break,
            None => {}
        }
        output.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(input: &str) -> String {
        let mut output = Vec::new();
        run(input.as_bytes(), &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn handshake() {
        let out = session("uci\nisready\nquit\n");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines,
            [
                format!("id name Bear {VERSION}").as_str(),
                "id author Ian Rastall",
                "uciok",
                "readyok"
            ]
        );
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
        assert_eq!(session("xyzzy\nucinewgame\n\n"), "");
    }
}
