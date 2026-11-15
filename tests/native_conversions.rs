//! Phase 3 — native codegen: str(), int(str), float(str).

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_nc_{name}_{}", std::process::id()));
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
fn native_str_int() {
    let (ok, out) = build_run("int", "print(str(42))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("42"), "got: {out}");
}

#[test]
fn native_str_float() {
    let (ok, out) = build_run("float", "print(str(3.14))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("3.14"), "got: {out}");
}

#[test]
fn native_str_bool() {
    let (ok, out) = build_run("bool", "print(str(true))\nprint(str(false))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("true"), "got: {out}");
    assert!(out.contains("false"), "got: {out}");
}

#[test]
fn native_int_from_str() {
    let (ok, out) = build_run("is", "print(int(\"123\"))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("123"), "got: {out}");
}

#[test]
fn native_float_from_str() {
    let (ok, out) = build_run("fs", "print(int(float(\"7.5\")))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("7"), "got: {out}");
}
