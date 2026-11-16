//! Phase 2 §3.7 — cross-module struct field access tests.

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_si_{name}_{}", std::process::id()));
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
fn imported_struct_field_access() {
    let (ok, out) = run(
        "field_access",
        &[
            ("pt.rca", "pub struct Point:\n    x: Int\n    y: Int\n"),
            (
                "main.rca",
                "import pt\nlet p = pt.Point(x = 1, y = 2)\nprint(p.x)\nprint(p.y)\n",
            ),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("1"), "got: {out}");
    assert!(out.contains("2"), "got: {out}");
}

#[test]
fn imported_struct_from_import_field_access() {
    let (ok, out) = run(
        "from_import",
        &[
            ("pt.rca", "pub struct Point:\n    x: Int\n"),
            (
                "main.rca",
                "from pt import Point\nlet p = Point(x = 5)\nprint(p.x)\n",
            ),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("5"), "got: {out}");
}

#[test]
fn qualified_type_in_fn_signature() {
    let (ok, out) = run(
        "qualified_type_sig",
        &[
            ("pt.rca", "pub struct Point:\n    x: Int\n"),
            (
                "main.rca",
                "import pt\nfn get_x(p: pt.Point) -> Int:\n    return p.x\nprint(get_x(pt.Point(x = 7)))\n",
            ),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("7"), "got: {out}");
}
