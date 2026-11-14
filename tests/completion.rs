//! §12 Phase 7 dev tool #9 — `rcac completion`.

use std::process::Command;

fn run(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(args)
        .output()
        .expect("spawn rcac");
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn completion_bash_contains_compdef() {
    let (ok, out) = run(&["completion", "bash"]);
    assert!(ok, "got: {out}");
    assert!(out.contains("_rcac()"), "got: {out}");
    assert!(out.contains("complete -F _rcac rcac"), "got: {out}");
}

#[test]
fn completion_zsh_contains_compdef() {
    let (ok, out) = run(&["completion", "zsh"]);
    assert!(ok);
    assert!(out.contains("#compdef rcac rca"), "got: {out}");
    assert!(out.contains("_describe"), "got: {out}");
}

#[test]
fn completion_fish_contains_complete() {
    let (ok, out) = run(&["completion", "fish"]);
    assert!(ok);
    assert!(out.contains("for cmd in rcac rca"), "got: {out}");
    assert!(out.contains("complete -c"), "got: {out}");
}

#[test]
fn completion_no_arg_errors() {
    let (ok, out) = run(&["completion"]);
    assert!(!ok);
    assert!(out.contains("usage"), "got: {out}");
}

#[test]
fn completion_unknown_shell_errors() {
    let (ok, out) = run(&["completion", "powershell"]);
    assert!(!ok);
    assert!(out.contains("unsupported shell"), "got: {out}");
}
