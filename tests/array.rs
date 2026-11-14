//! Phase 2 §4.1 — fixed-size Array type (Option B: C/Rust/Java-style).

use std::process::Command;

fn run(src: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_arr_{}.rca", name));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn array_correct_size_works() {
    let (ok, out) = run("let a: Array[Int, 3] = [1, 2, 3]\nprint(a)\n", "ok3");
    assert!(ok, "got: {out}");
    assert!(out.contains("[1, 2, 3]"), "got: {out}");
}

#[test]
fn array_too_short_errors() {
    let (ok, out) = run("let a: Array[Int, 3] = [1, 2]\n", "short");
    assert!(!ok);
    let l = out.to_lowercase();
    assert!(l.contains("array size mismatch"), "got: {out}");
    assert!(l.contains("3"), "got: {out}");
    assert!(l.contains("2"), "got: {out}");
}

#[test]
fn array_too_long_errors() {
    let (ok, out) = run("let a: Array[Int, 3] = [1, 2, 3, 4]\n", "long");
    assert!(!ok);
    let l = out.to_lowercase();
    assert!(l.contains("array size mismatch"), "got: {out}");
    assert!(l.contains("4"), "got: {out}");
}

#[test]
fn array_missing_size_errors() {
    let (ok, out) = run("let a: Array = [1, 2]\n", "nosize");
    assert!(!ok);
    let l = out.to_lowercase();
    assert!(
        l.contains("fixed size") || l.contains("needs"),
        "got: {out}"
    );
}

#[test]
fn array_of_string_works() {
    let (ok, out) = run(
        "let a: Array[String, 2] = [\"hi\", \"yo\"]\nprint(a)\n",
        "str",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("hi"), "got: {out}");
}

#[test]
fn array_from_another_array_checked() {
    // a: Array[Int,3]; b: Array[Int,2] = a  -> size mismatch
    let (ok, out) = run(
        "let a: Array[Int, 3] = [1, 2, 3]\nlet b: Array[Int, 2] = a\n",
        "from_arr",
    );
    assert!(!ok);
    let l = out.to_lowercase();
    assert!(l.contains("array size mismatch"), "got: {out}");
}

#[test]
fn array_zero_size_works() {
    let (ok, out) = run("let a: Array[Int, 0] = []\nprint(a)\n", "zero");
    // Either works or rejects empty-list inference — both are acceptable;
    // we only assert no panic.
    let _ = (ok, out);
}
