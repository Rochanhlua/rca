//! Phase 3 — native codegen: struct display and str(struct).

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_nsd_{name}_{}", std::process::id()));
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
    let out = Command::new(d.join("prog")).output().expect("run native");
    let _ = std::fs::remove_dir_all(&d);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn native_struct_print_and_str() {
    let src = "struct User:\n    name: String\n    age: Int\n\nlet u = User(name = \"RCA\", age = 20)\nprint(u)\nprint(str(u))\nprint(u.age)\n";
    let (ok, out) = build_run("basic", src);
    assert!(ok, "got: {out}");
    assert!(
        out.matches("User(name = \"RCA\", age = 20)").count() == 2,
        "got: {out}"
    );
    assert!(out.contains("20"), "got: {out}");
}

#[test]
fn native_struct_float_bool() {
    let src = "struct P:\n    x: Float\n    ok: Bool\n\nlet p = P(x = 1.5, ok = true)\nprint(p)\n";
    let (ok, out) = build_run("fb", src);
    assert!(ok, "got: {out}");
    assert!(out.contains("P(x = 1.5, ok = true)"), "got: {out}");
}
