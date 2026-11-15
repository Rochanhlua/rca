//! Phase 2 §3.2 — `private` visibility modifier.

use std::path::Path;
use std::process::Command;

fn run_cmd(cmd: &str, path: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args([cmd, path.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

/// Runs `src` with the VM and the tree interpreter; both must agree.
fn run_both(src: &str, name: &str) -> (bool, String) {
    let p = std::env::temp_dir().join(format!("rca_private_{}.rca", name));
    std::fs::write(&p, src).unwrap();
    let vm = run_cmd("run", &p);
    let tree = run_cmd("run-tree", &p);
    let _ = std::fs::remove_file(&p);
    assert_eq!(vm, tree, "VM and tree interpreter differ");
    vm
}

#[test]
fn private_fn_runs() {
    let (ok, out) = run_both(
        "private fn secret() -> Int:\n    return 42\n\nprint(secret())\n",
        "fn",
    );
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "42");
}

#[test]
fn private_struct_runs() {
    let (ok, out) = run_both(
        "private struct P:\n    x: Int\n\nlet p = P(x = 7)\nprint(p.x)\n",
        "struct",
    );
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "7");
}

#[test]
fn private_enum_parses() {
    let (ok, out) = run_both("private enum C:\n    A\n    B\n\nprint(1)\n", "enum");
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "1");
}

#[test]
fn pub_private_is_error() {
    let (ok, out) = run_both("pub private fn x() -> Int:\n    return 1\n", "pubpriv");
    assert!(!ok, "got: {out}");
    assert!(out.contains("after 'pub'"), "got: {out}");
}

#[test]
fn private_without_item_is_error() {
    let (ok, out) = run_both("private let x = 1\n", "bare");
    assert!(!ok, "got: {out}");
    assert!(out.contains("after 'private'"), "got: {out}");
}

#[test]
fn private_item_is_not_importable() {
    let dir = std::env::temp_dir().join("rca_private_mod_dir");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("pm.rca"),
        "private fn secret() -> Int:\n    return 1\n\npub fn visible() -> Int:\n    return 2\n",
    )
    .unwrap();

    let bad = dir.join("bad.rca");
    std::fs::write(&bad, "from pm import secret\nprint(secret())\n").unwrap();
    let (ok, out) = run_cmd("run", &bad);
    assert!(!ok, "got: {out}");
    assert!(out.contains("'secret' is private in module 'pm'"), "got: {out}");

    let good = dir.join("good.rca");
    std::fs::write(&good, "from pm import visible\nprint(visible())\n").unwrap();
    let (ok, out) = run_cmd("run", &good);
    assert!(ok, "got: {out}");
    assert_eq!(out.trim(), "2");

    let _ = std::fs::remove_dir_all(&dir);
}
