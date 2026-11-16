//! Phase 3 — native codegen: file read_file, write_file, file_exists.

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_nf_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn build_run(name: &str, src: &str) -> (bool, String) {
    let d = dir(name);
    let file = d.join("prog.rca");
    std::fs::write(&file, src).unwrap();
    let build = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["build", file.to_str().unwrap()])
        .output()
        .expect("spawn build");
    if !build.status.success() {
        let _ = std::fs::remove_dir_all(&d);
        return (false, String::from_utf8_lossy(&build.stderr).to_string());
    }
    let exe = d.join("prog");
    let out = Command::new(&exe).output().expect("run native");
    let _ = std::fs::remove_dir_all(&d);
    let code = out.status.code();
    let mut s = format!("[code={code:?}]\n");
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn native_file_write_then_read() {
    let (ok, out) = build_run(
        "wr",
        "import fs\nfs.write(\"rca_nf_test.txt\", \"hello\")\nlet s = fs.read(\"rca_nf_test.txt\")\nprint(s)\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("hello"), "got: {out}");
}

#[test]
fn native_file_exists_true() {
    let (ok, out) = build_run(
        "ex_t",
        "import fs\nfs.write(\"rca_nf_e.txt\", \"x\")\nif fs.exists(\"rca_nf_e.txt\"):\n    print(\"yes\")\nelse:\n    print(\"no\")\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("yes"), "got: {out}");
}

#[test]
fn native_file_exists_false() {
    let (ok, out) = build_run(
        "ex_f",
        "import fs\nif fs.exists(\"rca_nf_nope_xyz.txt\"):\n    print(\"yes\")\nelse:\n    print(\"no\")\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("no"), "got: {out}");
}
