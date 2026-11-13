//! Drives the built `bear` binary over real pipes.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn binary_answers_the_uci_handshake() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bear"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("bear should start");

    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(b"uci\nisready\nquit\n")
        .expect("bear should accept input");

    let output = child.wait_with_output().expect("bear should exit");
    assert!(output.status.success(), "exit status: {}", output.status);

    let text = String::from_utf8(output.stdout).expect("output is UTF-8");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "{text}");
    assert!(lines[0].starts_with("id name Bear "), "{text}");
    assert_eq!(lines[1], "id author Ian Rastall");
    assert_eq!(lines[2], "uciok");
    assert_eq!(lines[3], "readyok");
}
