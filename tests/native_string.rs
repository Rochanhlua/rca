//! Phase 3 — native codegen: string.trim(), string.replace().

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_ns_{name}_{}", std::process::id()));
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
    let out = Command::new(&exe)
        .current_dir(&d)
        .output()
        .expect("run native");
    let _ = std::fs::remove_dir_all(&d);
    let code = out.status.code();
    let mut s = format!("[code={code:?}]\n");
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn native_str_trim_leading_trailing() {
    let (ok, out) = build_run("trim", "import string\nprint(string.trim(\"  hi  \"))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("hi"), "got: {out}");
    assert!(
        !out.contains("  hi"),
        "trim did not remove leading space, got: {out}"
    );
}

#[test]
fn native_str_trim_no_change() {
    let (ok, out) = build_run("trim2", "import string\nprint(string.trim(\"abc\"))\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("abc"), "got: {out}");
}

#[test]
fn native_str_replace_basic() {
    let (ok, out) = build_run(
        "rep",
        "import string\nprint(string.replace(\"a-b-c\", \"-\", \"+\"))\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("a+b+c"), "got: {out}");
}

#[test]
fn native_str_replace_no_match() {
    let (ok, out) = build_run(
        "rep2",
        "import string\nprint(string.replace(\"hello\", \"x\", \"y\"))\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("hello"), "got: {out}");
}

#[test]
fn native_str_split_basic() {
    let (ok, out) = build_run(
        "split",
        "import string\nlet parts = string.split(\"a,b,c\", \",\")\nif len(parts) == 3:\n    print(\"ok\")\nelse:\n    print(\"bad\")\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("ok"), "got: {out}");
}

#[test]
fn native_str_split_single() {
    let (ok, out) = build_run(
        "split1",
        "import string\nlet parts = string.split(\"abc\", \",\")\nif len(parts) == 1:\n    print(\"ok\")\nelse:\n    print(\"bad\")\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("ok"), "got: {out}");
}

#[test]

fn native_push_char() {
    let (ok, out) = build_run("ch", "let c = 'a'\nprint(c)\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("a"), "got: {out}");
}
