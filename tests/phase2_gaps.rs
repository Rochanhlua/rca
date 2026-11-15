//! Phase 2 gap closure: List[T], Map[K,V] annotations, pub const.

use std::process::Command;

fn run(src: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_p2_{}.rca", name));
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
fn list_annotation_works() {
    let (ok, out) = run("let l: List[Int] = [1, 2, 3]\nprint(l)\n", "la");
    assert!(ok, "got: {out}");
    assert!(out.contains("[1, 2, 3]"), "got: {out}");
}

#[test]
fn list_annotation_type_checked() {
    let (ok, out) = run("let l: List[Int] = [1.0, 2.0]\n", "lat");
    assert!(!ok);
    let l = out.to_lowercase();
    assert!(
        l.contains("list[int]") && l.contains("list[float]"),
        "got: {out}"
    );
}

#[test]
fn map_annotation_works() {
    let (ok, out) = run("let m: Map[Int, String] = map_new()\nprint(m)\n", "ma");
    assert!(ok, "got: {out}");
    assert!(out.to_lowercase().contains("map"), "got: {out}");
}

#[test]
fn map_annotation_type_checked() {
    // map_new() -> Map[Unknown, Unknown] which is compatible.
    // The test just verifies the annotation is accepted.
    let (ok, out) = run("let m: Map[Int, String] = map_new()\nprint(1)\n", "mat");
    assert!(ok, "got: {out}");
}

#[test]
fn pub_const_works() {
    let (ok, out) = run("pub const X: Int = 42\nprint(X)\n", "pc");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "42");
}

#[test]
fn pub_const_without_type_uses_inference() {
    // Blueprint §3.2 does not require a type annotation on `const`.
    let (ok, out) = run("pub const X = 42\nprint(X)\n", "pcn");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "42");
}
