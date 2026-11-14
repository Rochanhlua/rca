//! §12 Phase 7 dev tool #6 — `rcac doc`.

use std::process::Command;

fn run(src: &str, name: &str) -> String {
    let p = std::env::temp_dir().join(format!("rca_doc_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["doc", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn doc_lists_public_fn() {
    let out = run(
        "pub fn add(a: Int, b: Int) -> Int:\n    return a + b\n",
        "fn",
    );
    assert!(out.contains("### add"), "got: {out}");
    assert!(out.contains("fn add(a: Int, b: Int) -> Int"), "got: {out}");
}

#[test]
fn doc_includes_leading_comment() {
    let out = run(
        "# Adds two numbers.\npub fn add(a: Int, b: Int) -> Int:\n    return a + b\n",
        "comment",
    );
    assert!(out.contains("Adds two numbers."), "got: {out}");
}

#[test]
fn doc_includes_public_struct() {
    let out = run("pub struct Point:\n    x: Int\n    y: Int\n", "struct");
    assert!(out.contains("### Point"), "got: {out}");
    assert!(out.contains("x: Int"), "got: {out}");
    assert!(out.contains("y: Int"), "got: {out}");
}

#[test]
fn doc_skips_private_items() {
    let out = run("fn hidden() -> Int:\n    return 1\n", "private");
    assert!(out.contains("_No public items found._"), "got: {out}");
    assert!(!out.contains("### hidden"), "got: {out}");
}

#[test]
fn doc_includes_public_enum() {
    let out = run("pub enum Color:\n    Red\n    Green\n    Blue\n", "enum");
    assert!(out.contains("### Color"), "got: {out}");
    assert!(out.contains("Red"), "got: {out}");
}
