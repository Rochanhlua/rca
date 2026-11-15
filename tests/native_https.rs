//! Phase 3 native codegen — HTTPS via FFI (rustls + librca.a).
//! Live test — enable with --ignored.

use std::process::Command;

#[test]
#[ignore = "live network; requires librca.a and clang"]
fn native_https_get_example_com() {
    let dir = std::env::temp_dir().join(format!("rca_native_https_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("h.rca");
    std::fs::write(
        &src,
        "import http\nprint(len(http.get(\"https://example.com/\")))\n",
    )
    .unwrap();
    let build = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["build", src.to_str().unwrap()])
        .output()
        .expect("spawn rcac build");
    let build_err = String::from_utf8_lossy(&build.stderr).to_string();
    assert!(build.status.success(), "build failed: {build_err}");
    let exe = dir.join("h");
    assert!(exe.exists(), "native binary not created");
    let out = Command::new(&exe).output().expect("run native");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "native exit {:?}: {stdout}",
        out.status.code()
    );
    assert!(stdout.contains("589"), "expected 589 bytes, got: {stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_build_links_librca() {
    // Ensure librca.a exists after build (verifies crate-type = staticlib).
    let dir = std::env::temp_dir().join(format!("rca_native_lnk_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("p.rca");
    std::fs::write(&src, "print(1 + 2)\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["build", src.to_str().unwrap()])
        .output()
        .expect("spawn rcac build");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "build failed: {stderr}");
    let exe = dir.join("p");
    assert!(exe.exists(), "native binary not created");
    let _ = std::fs::remove_dir_all(&dir);
}
