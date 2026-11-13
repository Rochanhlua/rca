use std::process::Command;

#[test]
fn test_sha256_builtin() {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("crypto_test.rca");
    std::fs::write(
        &temp_file,
        r#"
let h = sha256("abc")
print(h)
"#,
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .arg("run")
        .arg(&temp_file)
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    );

    let _ = std::fs::remove_file(temp_file);
}
