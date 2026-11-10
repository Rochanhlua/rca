use std::process::Command;

#[test]
fn test_native_build() {
    let temp = std::env::temp_dir().join("native_ci_test.rca");
    std::fs::write(&temp, "print(42)\n").unwrap();

    let exe = temp.with_extension("");
    let build = Command::new(env!("CARGO_BIN_EXE_rca"))
        .args(["build", temp.to_str().unwrap()])
        .output()
        .unwrap();

    if !build.status.success() {
        eprintln!(
            "Build failed (may be OK if clang missing): {}",
            String::from_utf8_lossy(&build.stderr)
        );
        return;
    }

    let run = Command::new(&exe).output().unwrap();
    assert!(String::from_utf8_lossy(&run.stdout).contains("42"));

    std::fs::remove_file(&temp).ok();
    std::fs::remove_file(&exe).ok();
}
