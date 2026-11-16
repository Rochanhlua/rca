//! Phase 3 — native codegen: time.now(), time.sleep(). (investigation)

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
    let code = out.status.code();
    let sig = {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            out.status.signal()
        }
        #[cfg(not(unix))]
        {
            None
        }
    };
    let mut s = format!("[code={code:?} signal={sig:?}]\n");
    s.push_str("stdout=");
    s.push_str(&String::from_utf8_lossy(&out.stdout));
    s.push_str("\nstderr=");
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn inv_print_only() {
    let (ok, out) = build_run("p", "print(\"hello\")\n");
    assert!(ok, "got: {out}");
    assert!(out.contains("hello"), "got: {out}");
}

#[test]
fn inv_sleep_zero() {
    let (ok, out) = build_run(
        "s0",
        "import time\nprint(\"before\")\ntime.sleep(0)\nprint(\"after\")\n",
    );
    eprintln!("inv_sleep_zero output: {out}");
    assert!(ok, "got: {out}");
    assert!(out.contains("before"), "got: {out}");
    assert!(out.contains("after"), "got: {out}");
}

#[test]
fn inv_sleep_one() {
    let (ok, out) = build_run(
        "s1",
        "import time\nprint(\"before\")\ntime.sleep(1)\nprint(\"after\")\n",
    );
    eprintln!("inv_sleep_one output: {out}");
    assert!(ok, "got: {out}");
    assert!(out.contains("before"), "got: {out}");
    assert!(out.contains("after"), "got: {out}");
}

#[test]
fn inv_now_works() {
    let (ok, out) = build_run(
        "n",
        "import time\nlet t = time.now()\nif t > 1600000000000:\n    print(\"ok\")\nelse:\n    print(\"bad\")\n",
    );
    assert!(ok, "got: {out}");
    assert!(out.contains("ok"), "got: {out}");
}
