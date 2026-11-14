//! §12 Phase 7 dev tool #2 — `rcac test`.

use std::path::PathBuf;
use std::process::Command;

fn repo_tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("rca")
}

#[test]
fn test_runner_passes_on_clean_subset() {
    // Point at a fresh temp dir with only passing tests.
    let dir = std::env::temp_dir().join(format!("rca_tr_pass_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("a.rca"),
        "let x = 1 + 1\nif x != 2:\n    throw \"bad\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("b.rca"),
        "let y = 3 * 3\nif y != 9:\n    throw \"bad\"\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["test", dir.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("PASS  "), "stdout: {stdout}");
    assert!(stdout.contains("2 passed, 0 failed"), "stdout: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_runner_reports_failure_and_nonzero_exit() {
    let dir = std::env::temp_dir().join(format!("rca_tr_fail_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("ok.rca"),
        "let x = 1\nif x != 1:\n    throw \"bad\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("bad.rca"), "throw \"intentional\"\n").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["test", dir.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(!out.status.success(), "expected failure exit");
    assert!(stdout.contains("PASS  "), "stdout: {stdout}");
    assert!(stdout.contains("FAIL  "), "stdout: {stdout}");
    assert!(stdout.contains("1 passed, 1 failed"), "stdout: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_runner_empty_dir_is_clean() {
    let dir = std::env::temp_dir().join(format!("rca_tr_empty_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["test", dir.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "stdout: {stdout}");
    assert!(stdout.contains("no .rca test files"), "stdout: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shipped_tests_directory_all_pass_except_failing_example() {
    // Run the actual tests/rca/ directory to keep it honest. One file
    // (failing_example.rca) is designed to fail.
    let dir = repo_tests_dir();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["test", dir.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(stdout.contains("PASS  "), "stdout: {stdout}");
    assert!(stdout.contains("failing_example"), "stdout: {stdout}");
    assert!(stdout.contains("1 failed"), "stdout: {stdout}");
}
