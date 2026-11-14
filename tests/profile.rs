//! §12 Phase 7 dev tool #12 (first slice) — `rcac profile`.
//! Foundation: VM stats hooks (VmStats) + `profile` subcommand.

use std::process::Command;

fn run(src: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_profile_{}.rca", name));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["profile", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn profile_prints_instruction_count() {
    let (ok, out) = run("print(1 + 2)\n", "instr");
    assert!(ok, "got: {out}");
    assert!(out.contains("profile:"), "got: {out}");
    assert!(out.contains("instructions:"), "got: {out}");
    assert!(out.contains("hottest lines:"), "got: {out}");
}

#[test]
fn profile_records_function_calls() {
    let (ok, out) = run(
        "fn add(a: Int, b: Int) -> Int:\n    return a + b\nlet r = add(1, 2)\nprint(r)\n",
        "func",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("functions by time:"), "got: {out}");
    assert!(out.contains("add"), "got: {out}");
    assert!(out.contains("(1 calls)"), "got: {out}");
}

#[test]
fn profile_imports_error() {
    let (ok, out) = run("import math\nprint(math.abs(-1))\n", "import");
    assert!(!ok, "expected failure");
    assert!(
        out.to_lowercase().contains("imports") || out.to_lowercase().contains("not support"),
        "got: {out}"
    );
}

#[test]
fn profile_parse_error_propagates() {
    let (ok, out) = run("let x =\n", "parse");
    assert!(!ok);
    assert!(!out.trim().is_empty(), "expected an error message");
}
