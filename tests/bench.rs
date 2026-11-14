//! §12 Phase 7 dev tool #7 — `rcac bench`.

use std::process::Command;

#[test]
fn bench_runs_and_reports() {
    let src = "print(1 + 2)\n";
    let p = std::env::temp_dir().join(format!("rca_bench_{}.rca", std::process::id()));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["bench", p.to_str().unwrap(), "3"])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("iterations: 3"), "stdout: {stdout}");
    assert!(stdout.contains("min:"), "stdout: {stdout}");
    assert!(stdout.contains("avg:"), "stdout: {stdout}");
    assert!(stdout.contains("max:"), "stdout: {stdout}");
}

#[test]
fn bench_default_is_five_iterations() {
    let src = "print(1)\n";
    let p = std::env::temp_dir().join(format!("rca_bench_d_{}.rca", std::process::id()));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["bench", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "stdout: {stdout}");
    assert!(stdout.contains("iterations: 5"), "stdout: {stdout}");
}

#[test]
fn bench_no_args_prints_usage() {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .arg("bench")
        .output()
        .expect("spawn rcac");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(stderr.contains("usage"), "stderr: {stderr}");
}

#[test]
fn bench_bad_iterations_fails() {
    let src = "print(1)\n";
    let p = std::env::temp_dir().join(format!("rca_bench_bad_{}.rca", std::process::id()));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["bench", p.to_str().unwrap(), "zero"])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success());
    assert!(stderr.contains("positive integer"), "stderr: {stderr}");
}
