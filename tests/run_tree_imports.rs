//! Phase 2 — run-tree (tree interpreter) module import support.

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_rt_{name}_{}", std::process::id()));
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
        .args(["run-tree", d.join(entry).to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_dir_all(&d);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn run_tree_imported_function() {
    let (ok, out) = run(
        "fn",
        &[
            ("lib.rca", "pub fn helper() -> Int:\n    return 42\n"),
            ("main.rca", "import lib\nprint(lib.helper())\n"),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("42"), "got: {out}");
}

#[test]
fn run_tree_from_import_function() {
    let (ok, out) = run(
        "from",
        &[
            (
                "lib.rca",
                "pub fn add(a: Int, b: Int) -> Int:\n    return a + b\n",
            ),
            ("main.rca", "from lib import add\nprint(add(3, 4))\n"),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("7"), "got: {out}");
}

#[test]
fn run_tree_imported_const() {
    let (ok, out) = run(
        "const",
        &[
            ("lib.rca", "pub const N: Int = 99\n"),
            ("main.rca", "import lib\nprint(lib.N)\n"),
        ],
        "main.rca",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("99"), "got: {out}");
}

#[test]
fn run_tree_imported_struct() {
    let (ok, out) = run(
        "struct",
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
fn run_tree_and_vm_agree() {
    let d = dir("agree");
    let lib = "pub fn inc(x: Int) -> Int:\n    return x + 1\n";
    let main = "import lib\nprint(lib.inc(10))\n";
    std::fs::write(d.join("lib.rca"), lib).unwrap();
    std::fs::write(d.join("main.rca"), main).unwrap();
    let entry = d.join("main.rca");
    let entry_s = entry.to_str().unwrap();
    let tree = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run-tree", entry_s])
        .output()
        .expect("spawn");
    let vm = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", entry_s])
        .output()
        .expect("spawn");
    let _ = std::fs::remove_dir_all(&d);
    let tree_out = String::from_utf8_lossy(&tree.stdout).to_string();
    let vm_out = String::from_utf8_lossy(&vm.stdout).to_string();
    assert!(
        tree.status.success(),
        "tree: {}",
        String::from_utf8_lossy(&tree.stderr)
    );
    assert!(
        vm.status.success(),
        "vm: {}",
        String::from_utf8_lossy(&vm.stderr)
    );
    assert_eq!(tree_out, vm_out, "tree vs VM mismatch");
}
