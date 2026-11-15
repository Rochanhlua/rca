//! Phase 2 §3.2 — module-qualified const access + from-import const tests.

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_mc_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(name: &str, files: &[(&str, &str)], entry: &str) -> (bool, String) {
    let d = dir(name);
    for (n, src) in files {
        std::fs::write(d.join(n), src).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", d.join(entry).to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_dir_all(&d);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn module_qualified_const_read() {
    let (ok, out) = run(
        "qualified",
        &[
            ("lib.rca", "pub const N: Int = 42\n"),
            ("main.rca", "import lib\nprint(lib.N)\n"),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("42"), "got: {out}");
}

#[test]
fn from_import_const_read() {
    let (ok, out) = run(
        "from",
        &[
            ("lib.rca", "pub const N: Int = 7\n"),
            ("main.rca", "from lib import N\nprint(N)\n"),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("7"), "got: {out}");
}

#[test]
fn from_import_const_with_alias() {
    let (ok, out) = run(
        "alias",
        &[
            ("lib.rca", "pub const N: Int = 5\n"),
            ("main.rca", "from lib import N as M\nprint(M)\n"),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("5"), "got: {out}");
}

#[test]
fn module_qualified_const_in_default_param() {
    let (ok, out) = run(
        "default",
        &[
            ("lib.rca", "pub const DEFAULT: Int = 99\n"),
            (
                "main.rca",
                "import lib\nfn f(a: Int = lib.DEFAULT) -> Int:\n    return a\nprint(f())\n",
            ),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("99"), "got: {out}");
}

#[test]
fn private_const_not_importable() {
    let (ok, out) = run(
        "private",
        &[
            ("lib.rca", "const HIDDEN: Int = 1\n"),
            ("main.rca", "from lib import HIDDEN\nprint(HIDDEN)\n"),
        ],
        "main.rca",
    );
    assert!(!ok, "expected error, got: {out}");
}
