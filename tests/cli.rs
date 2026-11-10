//! Integration tests for the `rca` CLI binary.
//!
//! Additive: does not modify `src/` or `examples/`.
//!
//! Run:  cargo test --test cli
//! All:  cargo test

use std::path::PathBuf;
use std::process::Command;

fn rca() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rca"))
}

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

// ---------- CLI surface ----------

#[test]
fn no_args_prints_usage() {
    let out = rca().output().expect("spawn rca");
    assert!(out.status.success(), "usage exit should be 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("RCA Programming Language"));
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("rca run"));
    assert!(stdout.contains("rca run-tree"));
}

// ---------- Happy path: hello.rca through every stage ----------

#[test]
fn hello_check_ok() {
    let out = rca()
        .args(["check"])
        .arg(example("hello.rca"))
        .output()
        .expect("spawn rca");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("OK: no errors"));
}

#[test]
fn hello_lex_ok() {
    let out = rca()
        .args(["lex"])
        .arg(example("hello.rca"))
        .output()
        .expect("spawn rca");
    assert!(out.status.success());
    assert!(!out.stdout.is_empty(), "lex should emit tokens");
}

#[test]
fn hello_parse_ok() {
    let out = rca()
        .args(["parse"])
        .arg(example("hello.rca"))
        .output()
        .expect("spawn rca");
    assert!(out.status.success());
    assert!(!out.stdout.is_empty(), "parse should emit an AST");
}

#[test]
fn hello_ir_ok() {
    let out = rca()
        .args(["ir"])
        .arg(example("hello.rca"))
        .output()
        .expect("spawn rca");
    assert!(out.status.success());
    assert!(!out.stdout.is_empty(), "ir should emit bytecode");
}

#[test]
fn hello_ir_opt_ok() {
    let out = rca()
        .args(["ir-opt"])
        .arg(example("hello.rca"))
        .output()
        .expect("spawn rca");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("optimized:"));
}

#[test]
fn hello_run_ok() {
    let out = rca()
        .args(["run"])
        .arg(example("hello.rca"))
        .output()
        .expect("spawn rca");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn hello_run_tree_ok() {
    let out = rca()
        .args(["run-tree"])
        .arg(example("hello.rca"))
        .output()
        .expect("spawn rca");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------- Error paths ----------

#[test]
fn bad_file_check_fails() {
    let out = rca()
        .args(["check"])
        .arg(example("bad.rca"))
        .output()
        .expect("spawn rca");
    assert!(
        !out.status.success(),
        "check on bad.rca should fail; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(!out.stderr.is_empty(), "expected a diagnostic on stderr");
}

#[test]
fn missing_file_reports_error() {
    let out = rca()
        .args(["run", "/nonexistent-rca-fixture.rca"])
        .output()
        .expect("spawn rca");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cannot read"), "stderr was: {}", stderr);
}

// ---------- Module loading ----------

#[test]
fn module_main_check_ok() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("modules")
        .join("main.rca");
    let out = rca().args(["check"]).arg(path).output().expect("spawn rca");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("OK: no errors"),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}
