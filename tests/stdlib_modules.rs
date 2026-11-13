//! §7 stdlib modules — `import math` etc. route to builtins.

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_std_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    p
}

fn run(src: &str, name: &str) -> String {
    let p = tmpfile(name, src);
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", p.to_str().unwrap()])
        .output()
        .expect("failed to spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn import_math_routes_to_builtins() {
    let src = "import math\nprint(math.abs(-5))\nprint(math.pow(2, 10))\n";
    let out = run(src, "math");
    assert!(out.contains("5\n"), "got: {out}");
    assert!(out.contains("1024"), "got: {out}");
}

#[test]
fn import_fs_routes_to_builtins() {
    let src = "import fs\nprint(fs.exists(\"Cargo.toml\"))\nprint(fs.exists(\"nope_xyz.txt\"))\n";
    let out = run(src, "fs");
    assert!(out.contains("true"), "got: {out}");
    assert!(out.contains("false"), "got: {out}");
}

#[test]
fn import_string_routes_to_builtins() {
    let src = "import string\nprint(string.trim(\"  hi  \"))\n";
    let out = run(src, "str");
    assert_eq!(out.trim(), "hi", "got: {out}");
}

#[test]
fn import_time_routes_to_builtins() {
    let src = "import time\nprint(time.now() > 0)\n";
    let out = run(src, "time");
    assert_eq!(out.trim(), "true", "got: {out}");
}

#[test]
fn import_json_routes_to_builtins() {
    let src = "import json\nprint(json.stringify(123))\n";
    let out = run(src, "json");
    assert_eq!(out.trim(), "123", "got: {out}");
}

#[test]
fn import_unknown_module_still_errors() {
    let src = "import nosuchmodule\nprint(1)\n";
    let out = run(src, "unknown");
    assert!(
        out.to_lowercase().contains("cannot find module"),
        "got: {out}"
    );
}
