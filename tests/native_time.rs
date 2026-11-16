//! Phase 3 — native codegen: time.now(), time.sleep().

use std::path::PathBuf;
use std::process::Command;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rca_nt_{name}_{}", std::process::id()));
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
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn native_time_now_returns_positive_int() {
    // time.now() returns ms since Unix epoch, which is > 1_600_000_000_000 (2020-09-13).
    let (ok, out) = build_run(
        "now",
        "import time\nlet t = time.now()\nif t > 1600000000000:\n    print(\"ok\")\nelse:\n    print(\"bad\")\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("ok"), "got: {out}");
}

#[test]
fn native_time_now_is_int() {
    let (ok, out) = build_run("now_int", "import time\nlet t = time.now()\nprint(t > 0)\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("true"), "got: {out}");
}

#[test]
fn native_time_sleep_does_not_crash() {
    let (ok, out) = build_run("sleep", "import time\ntime.sleep(1)\nprint(\"done\")\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("done"), "got: {out}");
}
