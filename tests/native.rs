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

    eprintln!("=== BUILD ===");
    eprintln!("exit: {:?}", build.status);
    eprintln!("stdout: {:?}", String::from_utf8_lossy(&build.stdout));
    eprintln!("stderr: {:?}", String::from_utf8_lossy(&build.stderr));
    eprintln!("exe path: {:?}", exe);
    eprintln!("exe exists: {}", exe.exists());

    if !build.status.success() {
        panic!("Build failed");
    }

    let run = Command::new(&exe).output().unwrap();
    eprintln!("=== RUN ===");
    eprintln!("exit: {:?}", run.status);
    eprintln!("stdout: {:?}", String::from_utf8_lossy(&run.stdout));
    eprintln!("stderr: {:?}", String::from_utf8_lossy(&run.stderr));
    eprintln!("===========");

    assert!(
        String::from_utf8_lossy(&run.stdout).contains("42"),
        "exe did not output 42"
    );

    std::fs::remove_file(&temp).ok();
    std::fs::remove_file(&exe).ok();
}
