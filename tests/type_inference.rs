//! Phase 2 §4.2 — type inference behavior tests.
//!
//! Verifies that RCA infers types from literals and expressions when
//! no annotation is given, and rejects conflicting inferred types.

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_infer_{name}.rca"));
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

// ─── Positive: literal inference ─────────────────────────────────────────

#[test]
fn infers_int_literal() {
    let (ok, out) = run("let x = 100\n", "int");
    assert!(ok, "got: {out}");
}

#[test]
fn infers_float_literal() {
    let (ok, out) = run("let y = 20.5\n", "float");
    assert!(ok, "got: {out}");
}

#[test]
fn infers_string_literal() {
    let (ok, out) = run("let s = \"hello\"\n", "string");
    assert!(ok, "got: {out}");
}

#[test]
fn infers_bool_literal() {
    let (ok, out) = run("let b = true\n", "bool");
    assert!(ok, "got: {out}");
}

#[test]
fn infers_char_literal() {
    let (ok, out) = run("let c = 'a'\n", "char");
    assert!(ok, "got: {out}");
}

#[test]
fn infers_list_of_int() {
    let (ok, out) = run("let xs = [1, 2, 3]\n", "list_int");
    assert!(ok, "got: {out}");
}

#[test]
fn infers_tuple() {
    let (ok, out) = run("let p = (1, \"a\")\n", "tuple");
    assert!(ok, "got: {out}");
}

// ─── Positive: expression inference ──────────────────────────────────────

#[test]
fn infers_binary_int_addition() {
    let (ok, out) = run("let a = 1\nlet b = 2\nlet c = a + b\n", "add_int");
    assert!(ok, "got: {out}");
}

#[test]
fn infers_comparison_as_bool() {
    let (ok, out) = run("let x = 1\nlet b = x < 2\nif b:\n    print(b)\n", "cmp");
    assert!(ok, "got: {out}");
}

// ─── Negative: conflicting inference ─────────────────────────────────────

#[test]
fn rejects_int_plus_float() {
    let (ok, out) = run("let x = 1\nlet y = 2.5\nlet z = x + y\n", "int_float");
    assert!(!ok, "expected error, got: {out}");
    assert!(out.contains("Int") && out.contains("Float"), "got: {out}");
}

#[test]
fn rejects_annotation_mismatch() {
    let (ok, out) = run("let x: Int = \"str\"\n", "ann_mismatch");
    assert!(!ok, "expected error, got: {out}");
}

#[test]
fn rejects_unknown_type_annotation() {
    let (ok, out) = run("let x: Foo = 1\n", "unknown_ty");
    assert!(!ok, "expected error, got: {out}");
    assert!(out.contains("unknown type"), "got: {out}");
}

#[test]
fn rejects_list_heterogeneous() {
    let (ok, out) = run("let xs = [1, \"a\"]\n", "het_list");
    assert!(!ok, "expected error, got: {out}");
}
