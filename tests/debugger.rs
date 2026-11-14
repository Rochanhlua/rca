//! §12 Phase 7 dev tool #10 — `rcac debugger`.

use std::io::Write;
use std::process::{Command, Stdio};

fn run_piped(src: &str, input: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_dbg_{}.rca", name));
    std::fs::write(&p, src).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["debugger", p.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rcac");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(input.as_bytes()).ok();
    }
    let out = child.wait_with_output().expect("wait");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn debugger_continue_runs_program() {
    let (ok, out) = run_piped("let x = 5\nprint(x)\n", ":continue\n", "run");
    assert!(ok, "got: {out}");
    assert!(out.contains("rcac debugger:"), "got: {out}");
    assert!(out.contains("paused at"), "got: {out}");
    assert!(out.contains("5"), "got: {out}");
}

#[test]
fn debugger_quit_exits_early() {
    let (ok, out) = run_piped("print(1)\nprint(2)\n", ":quit\n", "quit");
    assert!(ok, "got: {out}");
    assert!(out.contains("(dbg)"), "got: {out}");
}

#[test]
fn debugger_help_lists_commands() {
    let (ok, out) = run_piped("print(1)\n", ":help\n:continue\n", "help");
    assert!(ok, "got: {out}");
    assert!(out.contains(":continue"), "got: {out}");
    assert!(out.contains(":step"), "got: {out}");
    assert!(out.contains(":locals"), "got: {out}");
}

#[test]
fn debugger_breakpoint_pauses() {
    let src = "let x = 5\nprint(x)\nprint(99)\n";
    let (ok, out) = run_piped(src, ":b 3\n:continue\n:continue\n", "bp");
    assert!(ok, "got: {out}");
    assert!(out.contains("breakpoint at line 3"), "got: {out}");
}

#[test]
fn debugger_locals_and_stack() {
    let (ok, out) = run_piped(
        "let x = 5\nprint(x)\n",
        ":locals\n:stack\n:continue\n",
        "ls",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("locals"), "got: {out}");
    assert!(out.contains("stack"), "got: {out}");
}

#[test]
fn debugger_imports_error() {
    let (ok, out) = run_piped("import math\nprint(math.abs(-1))\n", ":continue\n", "imp");
    assert!(!ok, "got: {out}");
    assert!(out.to_lowercase().contains("imports"), "got: {out}");
}

#[test]
fn debugger_parse_error_propagates() {
    let (ok, out) = run_piped("let x =\n", "", "parse");
    assert!(!ok);
    assert!(!out.trim().is_empty());
}
