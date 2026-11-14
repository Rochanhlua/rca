//! §7 — package-aware module loader.

use std::path::{Path, PathBuf};
use std::process::Command;

fn setup(tag: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("rca_pkgload_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let proj = base.join("proj");
    let home = base.join("home");
    std::fs::create_dir_all(&proj).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    (proj, home)
}

fn run(dir: &Path, args: &[&str], home: &Path) -> (bool, String) {
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

fn install_lib(proj: &Path, home: &Path, lib_src: &str) -> PathBuf {
    let lib = proj.join("lib.rca");
    std::fs::write(&lib, lib_src).unwrap();
    assert!(run(proj, &["pkg", "init"], home).0);
    assert!(run(proj, &["pkg", "add", "mylib", lib.to_str().unwrap()], home).0);
    let (ok, out) = run(proj, &["pkg", "install"], home);
    assert!(ok, "install failed: {out}");
    home.join(".rca")
        .join("packages")
        .join("mylib")
        .join("main.rca")
}

#[test]
fn import_resolves_from_installed_package() {
    let (proj, home) = setup("basic");
    let _lib = install_lib(
        &proj,
        &home,
        "pub fn greet(n: String) -> String:\n    return \"hi \" + n\n",
    );
    let main = proj.join("main.rca");
    std::fs::write(&main, "import mylib\nprint(mylib.greet(\"x\"))\n").unwrap();
    let (ok, out) = run(&proj, &["run", "main.rca"], &home);
    assert!(ok, "run failed: {out}");
    assert!(out.contains("hi x"), "got: {out}");
    let _ = std::fs::remove_dir_all(proj.parent().unwrap());
}

#[test]
fn local_file_takes_priority_over_package() {
    let (proj, home) = setup("prio");
    let _lib = install_lib(
        &proj,
        &home,
        "pub fn who() -> String:\n    return \"from-package\"\n",
    );
    // Create a local mylib.rca that should win
    std::fs::write(
        proj.join("mylib.rca"),
        "pub fn who() -> String:\n    return \"from-local\"\n",
    )
    .unwrap();
    let main = proj.join("main.rca");
    std::fs::write(&main, "import mylib\nprint(mylib.who())\n").unwrap();
    let (ok, out) = run(&proj, &["run", "main.rca"], &home);
    assert!(ok, "run failed: {out}");
    assert!(out.contains("from-local"), "got: {out}");
    let _ = std::fs::remove_dir_all(proj.parent().unwrap());
}

#[test]
fn missing_module_lists_both_paths() {
    let (proj, home) = setup("missing");
    let main = proj.join("main.rca");
    std::fs::write(&main, "import nothere\nprint(1)\n").unwrap();
    let (ok, out) = run(&proj, &["run", "main.rca"], &home);
    assert!(!ok);
    assert!(out.contains("cannot find module"), "got: {out}");
    assert!(out.contains("nothere.rca"), "got: {out}");
    let _ = std::fs::remove_dir_all(proj.parent().unwrap());
}

#[test]
fn no_home_still_reports_project_path() {
    let (proj, _home) = setup("nohome");
    let main = proj.join("main.rca");
    std::fs::write(&main, "import missing_x\nprint(1)\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .current_dir(&proj)
        .env_remove("HOME")
        .args(["run", "main.rca"])
        .output()
        .expect("spawn rcac");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!out.status.success());
    assert!(combined.contains("cannot find module"), "got: {combined}");
    let _ = std::fs::remove_dir_all(proj.parent().unwrap());
}
