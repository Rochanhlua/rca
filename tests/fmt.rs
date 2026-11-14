//! §12 Phase 7 dev tool #3 — `rcac fmt`.

use std::process::Command;

fn run_fmt(src: &str, name: &str) -> String {
    let p = std::env::temp_dir().join(format!("rca_fmt_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["fmt", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn fmt_preserves_comment() {
    let out = run_fmt("# top\nlet x = 5\nprint(x)\n", "comment");
    assert!(out.contains("# top"), "got: {out}");
}

#[test]
fn fmt_keeps_let_and_assign() {
    let out = run_fmt("let x=5\n", "space");
    assert!(out.contains("let x = 5"), "got: {out}");
}

#[test]
fn fmt_idempotent() {
    let src = "let x = 5\nprint(x)\n";
    let once = run_fmt(src, "idem1");
    let twice = run_fmt(&once, "idem2");
    assert_eq!(once.trim_end(), twice.trim_end());
}

#[test]
fn fmt_4_space_indent() {
    let out = run_fmt("fn f() -> Int:\n    return 1\n", "indent");
    assert!(out.contains("    return 1"), "got: {out}");
}
