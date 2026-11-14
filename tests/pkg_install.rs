//! §7 — `rcac pkg install`.

use std::process::Command;

fn setup(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rca_pkg_inst_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(dir: &std::path::Path, args: &[&str], home: &std::path::Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .current_dir(dir)
        .env("HOME", home)
        .args(args)
        .output()
        .expect("spawn rcac");
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

#[test]
fn pkg_install_local_path() {
    let dir = setup("local");
    let home = setup("localhome");
    let src = dir.join("lib.rca");
    std::fs::write(&src, "fn hello() -> String:\n    return \"hi\"\n").unwrap();
    assert!(run(&dir, &["pkg", "init"], &home).0);
    assert!(run(&dir, &["pkg", "add", "mylib", src.to_str().unwrap()], &home).0);
    let (ok, out) = run(&dir, &["pkg", "install"], &home);
    assert!(ok, "got: {out}");
    assert!(out.contains("installed 1 / 1"), "got: {out}");
    let dest = home
        .join(".rca")
        .join("packages")
        .join("mylib")
        .join("main.rca");
    let content = std::fs::read_to_string(&dest).expect("main.rca missing");
    assert!(content.contains("hi"), "content: {content}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn pkg_install_https_dispatches_via_rustls() {
    // https:// now routes through http_get (rustls). Use a host that fails
    // DNS so we can assert the scheme was accepted (not rejected by the
    // scheme check) without making a live network request.
    let dir = setup("https");
    let home = setup("httpshome");
    assert!(run(&dir, &["pkg", "init"], &home).0);
    assert!(
        run(
            &dir,
            &["pkg", "add", "x", "https://nonexistent.invalid/x.rca"],
            &home
        )
        .0
    );
    let (ok, out) = run(&dir, &["pkg", "install"], &home);
    assert!(!ok, "expected network failure");
    assert!(!out.contains("not yet supported"), "scheme rejected: {out}");
    // The failure should be a network/connect error from the https path.
    let lower = out.to_lowercase();
    assert!(
        lower.contains("connect failed") || lower.contains("dns") || lower.contains("lookup"),
        "got: {out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn pkg_install_rejects_github() {
    let dir = setup("gh");
    let home = setup("ghhome");
    assert!(run(&dir, &["pkg", "init"], &home).0);
    assert!(run(&dir, &["pkg", "add", "x", "github:u/r"], &home).0);
    let (ok, out) = run(&dir, &["pkg", "install"], &home);
    assert!(!ok);
    assert!(out.contains("github: sources not yet"), "got: {out}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn pkg_install_no_manifest_errors() {
    let dir = setup("nomani");
    let home = setup("nomanihome");
    let (ok, out) = run(&dir, &["pkg", "install"], &home);
    assert!(!ok);
    assert!(out.contains("no rca.json"), "got: {out}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn pkg_install_empty_deps() {
    let dir = setup("empty");
    let home = setup("emptyhome");
    assert!(run(&dir, &["pkg", "init"], &home).0);
    let (ok, out) = run(&dir, &["pkg", "install"], &home);
    assert!(ok, "got: {out}");
    assert!(out.contains("no dependencies"), "got: {out}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&home);
}
