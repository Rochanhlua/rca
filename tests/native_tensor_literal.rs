use std::process::Command;

/// Native Tensor literal + shape() must match the VM.
#[test]
fn native_tensor_literal_shape_matches_vm() {
    let dir = std::env::temp_dir();
    let src = dir.join("native_tensor_literal_test.rca");
    std::fs::write(
        &src,
        "let a = Tensor([[1.0, 2.0], [3.0, 4.0]])\nprint(shape(a))\n",
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
    let native = String::from_utf8_lossy(&run.stdout).replace("\r\n", "\n");

    let vm = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", src.to_str().unwrap()])
        .output()
        .unwrap();
    let vm_out = String::from_utf8_lossy(&vm.stdout).replace("\r\n", "\n");

    assert_eq!(native.trim(), "[2, 2]", "native output: {native:?}");
    assert_eq!(native.trim(), vm_out.trim(), "native differs from VM");

    std::fs::remove_file(&src).ok();
    std::fs::remove_file(&exe).ok();
}
