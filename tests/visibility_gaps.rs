//! Phase 2 §3.2 — private let, private const, pub let.

use std::process::Command;

fn run(src: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_vis_{}.rca", name));
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
fn private_let_works() {
    let (ok, out) = run("private let x: Int = 42\nprint(x)\n", "pl");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "42");
}

#[test]
fn private_const_works() {
    let (ok, out) = run("private const Y: Int = 7\nprint(Y)\n", "pc");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "7");
}

#[test]
fn pub_let_works() {
    let (ok, out) = run("pub let Z: Int = 99\nprint(Z)\n", "pbl");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "99");
}

#[test]
fn pub_const_still_works() {
    let (ok, out) = run("pub const W: Int = 5\nprint(W)\n", "pbc");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "5");
}
