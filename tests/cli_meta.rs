//! §7 — rcac --version / --help meta commands.

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
fn version_prints_semver() {
    let (ok, out) = run(&["--version"]);
    assert!(ok, "got: {out}");
    assert!(out.starts_with("rcac "), "got: {out}");
    assert!(out.contains('.'), "got: {out}");
}

#[test]
fn version_short_flag() {
    let (ok, out) = run(&["-V"]);
    assert!(ok);
    assert!(out.starts_with("rcac "), "got: {out}");
}

#[test]
fn help_lists_commands() {
    let (ok, out) = run(&["--help"]);
    assert!(ok, "got: {out}");
    assert!(out.contains("rcac <command>"), "got: {out}");
    assert!(out.contains("lex"), "got: {out}");
    assert!(out.contains("run-tree"), "got: {out}");
    assert!(out.contains("pkg"), "got: {out}");
}

#[test]
fn help_short_flag() {
    let (ok, out) = run(&["-h"]);
    assert!(ok);
    assert!(out.contains("rcac <command>"), "got: {out}");
}

#[test]
fn no_args_still_prints_usage() {
    let (ok, out) = run(&[]);
    assert!(ok, "got: {out}");
    assert!(out.contains("Usage"), "got: {out}");
}
