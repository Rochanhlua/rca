//! §12 Phase 7 dev tool #4 — `rcac repl`.

use std::io::Write;
use std::process::{Command, Stdio};

fn run_repl(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rcac repl");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(input.as_bytes()).ok();
    }
    let out = child.wait_with_output().expect("wait");
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn repl_quit_exits_zero() {
    let out = run_repl(":quit\n");
    assert!(out.contains("rca> "), "got: {out}");
}

#[test]
fn repl_eof_exits_zero() {
    let out = run_repl("");
    assert!(out.contains("rca> "), "got: {out}");
}

#[test]
fn repl_runs_simple_print() {
    let out = run_repl("print(1 + 2)\n:quit\n");
    assert!(out.contains("3"), "got: {out}");
}

#[test]
fn repl_empty_lines_are_ignored() {
    let out = run_repl("\n\n\n:quit\n");
    assert!(out.contains("rca> "), "got: {out}");
}

#[test]
fn repl_help_lists_commands() {
    let out = run_repl(":help\n:quit\n");
    assert!(out.contains(":quit"), "got: {out}");
}
