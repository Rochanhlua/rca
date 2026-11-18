//! Phase 3 — native codegen: str() of list and tuple.

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_nss_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn build_run(name: &str, src: &str) -> (bool, String) {
    let d = dir(name);
    let file = d.join("prog.rca");
    std::fs::write(&file, src).unwrap();
    let build = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["build", file.to_str().unwrap()])
        .output()
        .expect("spawn build");
    if !build.status.success() {
        let _ = std::fs::remove_dir_all(&d);
        return (false, String::from_utf8_lossy(&build.stderr).to_string());
    }
    let exe = d.join("prog");
    let out = Command::new(&exe).output().expect("run native");
    let _ = std::fs::remove_dir_all(&d);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn native_str_list_int() {
    let (ok, out) = build_run("li", "print(str([1, 2, 3]))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("[1, 2, 3]"), "got: {out}");
}

#[test]
fn native_str_list_float() {
    let (ok, out) = build_run("lf", "print(str([1.5, 2.0]))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("[1.5, 2.0]"), "got: {out}");
}

#[test]
fn native_str_list_str() {
    let (ok, out) = build_run("ls", "print(str([\"a\", \"b\"]))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("[\"a\", \"b\"]"), "got: {out}");
}

#[test]
fn native_str_list_bool() {
    let (ok, out) = build_run("lb", "print(str([true, false]))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("[true, false]"), "got: {out}");
}

#[test]
fn native_str_tuple_mixed() {
    let (ok, out) = build_run("tm", "print(str((1, \"x\", 2.5, true)))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("(1, \"x\", 2.5, true)"), "got: {out}");
}
