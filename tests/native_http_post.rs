//! Phase 3 native codegen — HttpPost via FFI (rustls).
//! Live test — enable with --ignored.

use std::process::Command;

#[test]
#[ignore = "live network; requires librca.a and clang"]
fn native_http_post_example_com() {
    let dir = std::env::temp_dir().join(format!("rca_native_post_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("h.rca");
    std::fs::write(
        &src,
        "import http\nlet r = http.post(\"https://example.com/\", \"hello\")\nprint(len(r))\n",
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
    // example.com returns a 405/200 body of some length; just require a number.
    let trimmed = stdout.trim();
    assert!(!trimmed.is_empty(), "empty output");
    assert!(
        trimmed.chars().all(|c| c.is_ascii_digit()),
        "expected a number, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_build_with_http_post_links() {
    // `cargo test` alone does not build the staticlib; `cargo build` does.
    // Skip when librca.a is not present so CI does not fail spuriously.
    let librca = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("librca.a");
    if !librca.exists() {
        eprintln!("skipping: librca.a not built (run `cargo build` first)");
        return;
    }
    // Ensure a program containing HttpPost builds and links cleanly.
    let dir = std::env::temp_dir().join(format!("rca_native_post_lnk_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("p.rca");
    std::fs::write(
        &src,
        "import http\nlet r = http.post(\"https://127.0.0.1:1/x\", \"hi\")\nprint(r)\n",
    )
    .unwrap();
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
