//! §7 — `rcac pkg add`.

use std::process::Command;

fn setup(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rca_pkg_add_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(dir: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("spawn rcac");
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn pkg_add_inserts_dependency() {
    let dir = setup("insert");
    assert!(run(&dir, &["pkg", "init"]).0);
    let (ok, out) = run(
        &dir,
        &["pkg", "add", "utils", "https://example.com/utils.rca"],
    );
    assert!(ok, "got: {out}");
    let m = std::fs::read_to_string(dir.join("rca.json")).unwrap();
    assert!(m.contains("\"utils\""), "m: {m}");
    assert!(m.contains("https://example.com/utils.rca"), "m: {m}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pkg_add_updates_existing_without_duplicate() {
    let dir = setup("update");
    assert!(run(&dir, &["pkg", "init"]).0);
    assert!(run(&dir, &["pkg", "add", "utils", "old.rca"]).0);
    assert!(run(&dir, &["pkg", "add", "utils", "new.rca"]).0);
    let m = std::fs::read_to_string(dir.join("rca.json")).unwrap();
    assert!(!m.contains("old.rca"), "m: {m}");
    assert!(m.contains("new.rca"), "m: {m}");
    assert_eq!(m.matches("\"utils\"").count(), 1, "dup key: {m}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pkg_add_without_manifest_errors() {
    let dir = setup("nomanifest");
    let (ok, out) = run(&dir, &["pkg", "add", "x", "y"]);
    assert!(!ok, "expected failure, got: {out}");
    assert!(out.contains("no rca.json"), "got: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pkg_add_arity_error() {
    let dir = setup("arity");
    let (ok, out) = run(&dir, &["pkg", "add", "onlyname"]);
    assert!(!ok);
    assert!(out.contains("usage"), "got: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}
