//! Phase 2 §3.5 — match return flow tests.

use std::process::Command;

fn run(src: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_mr_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["check", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn match_all_arms_return_passes() {
    let src = "fn f(x: Int) -> Int:\n    match x:\n        1:\n            return 10\n        _:\n            return 20\n";
    let (ok, out) = run(src, "all");
    assert!(ok, "got: {out}");
}

#[test]
fn match_some_arm_missing_return_fails() {
    let src = "fn f(x: Int) -> Int:\n    match x:\n        1:\n            return 10\n        _:\n            print(0)\n";
    let (ok, out) = run(src, "some");
    assert!(!ok, "expected error, got: {out}");
    assert!(out.contains("must return"), "got: {out}");
}

#[test]
fn exhaustive_enum_match_all_return_passes() {
    let src = "enum C:\n    A\n    B\nfn f(c: C) -> Int:\n    match c:\n        C.A:\n            return 1\n        C.B:\n            return 2\n";
    let (ok, out) = run(src, "enum");
    assert!(ok, "got: {out}");
}

#[test]
fn if_else_match_combo_passes() {
    let src = "fn f(x: Int) -> Int:\n    if x > 0:\n        return 1\n    else:\n        match x:\n            0:\n                return 0\n            _:\n                return -1\n";
    let (ok, out) = run(src, "combo");
    assert!(ok, "got: {out}");
}

#[test]
fn match_then_trailing_return_passes() {
    let src = "fn f(x: Int) -> Int:\n    match x:\n        1:\n            print(1)\n        _:\n            print(2)\n    return 0\n";
    let (ok, out) = run(src, "trail");
    assert!(ok, "got: {out}");
}
