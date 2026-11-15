//! Phase 2 §3.2 — cross-module enum import tests.

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_ei_{name}_{}", std::process::id()));
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
        .args(["check", d.join(entry).to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_dir_all(&d);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn from_import_enum_works() {
    let (ok, out) = run(
        "from_import",
        &[
            ("c.rca", "pub enum Color:\n    Red\n    Green\n"),
            (
                "main.rca",
                "from c import Color\nlet x = Color.Red\nmatch x:\n    Color.Red:\n        print(\"r\")\n    Color.Green:\n        print(\"g\")\n",
            ),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
}

#[test]
fn from_import_enum_type_annotation() {
    let (ok, out) = run(
        "type_ann",
        &[
            ("c.rca", "pub enum Color:\n    Red\n    Green\n"),
            (
                "main.rca",
                "from c import Color\nlet x: Color = Color.Green\nprint(x)\n",
            ),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
}

#[test]
fn private_enum_not_importable() {
    let (ok, out) = run(
        "private",
        &[
            ("c.rca", "enum Secret:\n    A\n    B\n"),
            ("main.rca", "from c import Secret\nlet x = Secret.A\n"),
        ],
        "main.rca",
    );
    assert!(!ok, "expected error, got: {out}");
}
