//! Phase 2 §4.1 — Char literal.

use std::process::Command;

fn run(src: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_char_{}.rca", name));
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
fn char_basic() {
    let (ok, out) = run("let c: Char = 'a'\nprint(c)\n", "basic");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "a");
}

#[test]
fn char_escape_newline() {
    let (ok, out) = run("let c: Char = '\\n'\nprint(c)\n", "nl");
    assert!(ok, "got: {out}");
    // Output is a newline + program's own newline.
    assert!(out.contains('\n'));
}

#[test]
fn char_in_struct() {
    let (ok, out) = run(
        "struct P:\n    c: Char\nlet p = P(c = 'x')\nprint(p.c)\n",
        "struct",
    );
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "x");
}

#[test]
fn char_in_list() {
    let (ok, out) = run("let cs = ['a', 'b', 'c']\nprint(cs[1])\n", "list");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "b");
}

#[test]
fn char_unterminated_errors() {
    let (ok, out) = run("let c: Char = 'a\n", "unterm");
    assert!(!ok);
    assert!(
        out.to_lowercase().contains("unterminated character"),
        "got: {out}"
    );
}

#[test]
fn char_type_mismatch_errors() {
    let (ok, out) = run("let c: Char = 5\n", "mismatch");
    assert!(!ok);
    assert!(
        out.to_lowercase().contains("type") || out.contains("Char"),
        "got: {out}"
    );
}
