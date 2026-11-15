//! Phase 2 §3.3 — variable declaration and mutation tests.
//!
//! Covers: let / mut / const, explicit annotation, shadowing,
//! block scope, mutability enforcement.

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_var_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    p
}

fn run(src: &str, name: &str) -> (bool, String) {
    let p = tmpfile(name, src);
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["check", p.to_str().unwrap()])
        .output()
        .expect("failed to spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

// ─── Positive ────────────────────────────────────────────────────────────

#[test]
fn let_without_annotation() {
    let (ok, out) = run("let x = 1\n", "l1");
    assert!(ok, "got: {out}");
}

#[test]
fn let_with_annotation() {
    let (ok, out) = run("let x: Int = 1\n", "l2");
    assert!(ok, "got: {out}");
}

#[test]
fn mut_allows_reassignment() {
    let (ok, out) = run("mut x = 1\nx = 2\n", "m1");
    assert!(ok, "got: {out}");
}

#[test]
fn const_declaration() {
    let (ok, out) = run("const C: Int = 42\n", "c1");
    assert!(ok, "got: {out}");
}

#[test]
fn shadowing_in_inner_scope() {
    let src = "let x = 1\nif true:\n    let x = 2\n    print(x)\nprint(x)\n";
    let (ok, out) = run(src, "sh1");
    assert!(ok, "got: {out}");
}

// ─── Negative ────────────────────────────────────────────────────────────

#[test]
fn let_reassignment_rejected() {
    let (ok, out) = run("let x = 1\nx = 2\n", "lr");
    assert!(!ok, "expected error, got: {out}");
    assert!(
        out.contains("immutable") || out.contains("cannot assign"),
        "got: {out}"
    );
}

#[test]
fn const_reassignment_rejected() {
    let (ok, out) = run("const C: Int = 1\nC = 2\n", "cr");
    assert!(!ok, "expected error, got: {out}");
    assert!(out.contains("constant"), "got: {out}");
}

#[test]
fn annotation_type_mismatch() {
    let (ok, out) = run("let x: Int = \"s\"\n", "am");
    assert!(!ok, "expected error, got: {out}");
}

#[test]
fn undefined_variable_use() {
    let (ok, out) = run("print(y)\n", "undef");
    assert!(!ok, "expected error, got: {out}");
}

#[test]
fn out_of_scope_rejected() {
    let src = "if true:\n    let inner = 1\nprint(inner)\n";
    let (ok, out) = run(src, "oos");
    assert!(!ok, "expected error, got: {out}");
}
