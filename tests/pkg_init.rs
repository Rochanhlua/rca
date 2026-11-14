//! §7 — `rcac pkg init`.

use std::process::Command;

#[test]
fn pkg_init_creates_manifest() {
    let dir = std::env::temp_dir().join(format!("rca_pkg_init_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .current_dir(&dir)
        .args(["pkg", "init"])
        .output()
        .expect("spawn rcac");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stdout: {stdout}\nstderr: {stderr}");
    let manifest = std::fs::read_to_string(dir.join("rca.json")).expect("rca.json missing");
    assert!(manifest.contains("\"name\""), "manifest: {manifest}");
    assert!(
        manifest.contains("\"version\": \"0.1.0\""),
        "manifest: {manifest}"
    );
    assert!(
        manifest.contains("\"dependencies\": {}"),
        "manifest: {manifest}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pkg_init_refuses_overwrite() {
    let dir = std::env::temp_dir().join(format!("rca_pkg_dup_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("rca.json"), "existing").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .current_dir(&dir)
        .args(["pkg", "init"])
        .output()
        .expect("spawn rcac");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "expected failure");
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}
