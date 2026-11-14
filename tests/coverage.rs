//! §12 Phase 7 dev tool #11 — `rcac coverage`.

use std::process::Command;

fn run(src: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_cov_{}.rca", name));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["coverage", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn coverage_fully_covered_is_100() {
    let (ok, out) = run("let x = 5\nprint(x)\n", "full");
    assert!(ok, "got: {out}");
    assert!(out.contains("coverage:"), "got: {out}");
    assert!(out.contains("100.0%"), "got: {out}");
    assert!(out.contains("missed:       0"), "got: {out}");
}

#[test]
fn coverage_unreachable_function_is_missed() {
    let (ok, out) = run(
        "fn add(a: Int, b: Int) -> Int:\n    return a + b\nlet r = add(1, 2)\nprint(r)\nfn unused() -> Int:\n    return 99\n",
        "unused",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("missed lines:"), "got: {out}");
    assert!(out.contains("fn unused"), "got: {out}");
    assert!(!out.contains("100.0%"), "got: {out}");
}

#[test]
fn coverage_imports_error() {
    let (ok, out) = run("import math\nprint(math.abs(-1))\n", "import");
    assert!(!ok);
    assert!(
        out.to_lowercase().contains("imports") || out.to_lowercase().contains("not support"),
        "got: {out}"
    );
}

#[test]
fn coverage_parse_error_propagates() {
    let (ok, out) = run("let x =\n", "parse");
    assert!(!ok);
    assert!(!out.trim().is_empty());
}
