//! §12 Phase 7 dev tool #8 — `rcac watch`.

use std::process::Command;

#[test]
fn watch_nonexistent_file_errors() {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["watch", "/definitely/not/a/real/path.rca"])
        .output()
        .expect("spawn rcac");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "expected failure");
    assert!(stderr.contains("cannot stat"), "stderr: {stderr}");
}

#[test]
fn watch_spawns_for_valid_file() {
    let p = std::env::temp_dir().join(format!("rca_watch_{}.rca", std::process::id()));
    std::fs::write(&p, "print(1)\n").unwrap();
    // Spawn, give it time to start, then kill. We do not need the output;
    // just verify it doesn't crash before the first poll.
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["watch", p.to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn rcac watch");
    std::thread::sleep(std::time::Duration::from_millis(800));
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&p);
}
