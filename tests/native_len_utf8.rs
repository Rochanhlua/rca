use std::process::Command;

/// Native `len(String)` must count characters, like the VM, not bytes.
#[test]
fn native_len_counts_utf8_characters() {
    let dir = std::env::temp_dir();
    let src = dir.join("native_len_utf8_test.rca");
    std::fs::write(
        &src,
        "print(len(\"a\u{e9}\u{4e16}z\"))\nprint(len(\"abc\"))\nprint(len(\"\"))\n",
    )
    .unwrap();

    let exe = src.with_extension("");
    let build = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["build", src.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&exe).output().unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n");

    let vm = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", src.to_str().unwrap()])
        .output()
        .unwrap();
    let vm_out = String::from_utf8_lossy(&vm.stdout).replace("\r\n", "\n");

    assert_eq!(stdout.trim(), "4\n3\n0", "native output: {stdout:?}");
    assert_eq!(stdout.trim(), vm_out.trim(), "native differs from VM");

    std::fs::remove_file(&src).ok();
    std::fs::remove_file(&exe).ok();
}
