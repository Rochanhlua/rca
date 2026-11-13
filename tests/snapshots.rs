//! Snapshot tests for `rca` CLI output format.
//!
//! Locks `rca lex`, `rca parse`, `rca ir`, `rca ir-opt` output so
//! accidental formatting changes are caught in CI (Phase 8:
//! "Compiler regression testing").
//!
//! Update goldens (ONLY when the change is intentional):
//!   RCA_UPDATE_SNAPSHOTS=1 cargo test --test snapshots
//! Then: `git add tests/snapshots/` and commit.
//!
//! Additive: this file + goldens only. No changes to src/, examples/,
//! Cargo.toml, or CI.
//!
//! Run: `cargo test --test snapshots`
//! All: `cargo test`

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn rca() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rcac"))
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn golden_path(name: &str) -> PathBuf {
    manifest_dir().join("tests").join("snapshots").join(name)
}

fn update_mode() -> bool {
    std::env::var("RCA_UPDATE_SNAPSHOTS").as_deref() == Ok("1")
}

fn check_snapshot(name: &str, command: &str, file: &str) {
    let out = rca()
        .arg(command)
        .arg(manifest_dir().join("examples").join(file))
        .output()
        .expect("spawn rca");
    assert!(
        out.status.success(),
        "rca {} {} failed: {}",
        command,
        file,
        String::from_utf8_lossy(&out.stderr)
    );
    let actual = String::from_utf8_lossy(&out.stdout)
        .replace("\r\n", "\n")
        .to_string();
    let path = golden_path(name);

    if update_mode() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &actual).unwrap();
        eprintln!("updated golden: {}", path.display());
        return;
    }

    let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {}\n\
             Run `RCA_UPDATE_SNAPSHOTS=1 cargo test --test snapshots` to create it.",
            path.display(),
            e
        )
    });

    if actual != expected {
        let mut diff = String::from("(no differing line found)");
        for (i, (a, e)) in actual.lines().zip(expected.lines()).enumerate() {
            if a != e {
                diff = format!(
                    "first diff at line {}:\n  expected: {}\n  actual:   {}",
                    i + 1,
                    e,
                    a
                );
                break;
            }
        }
        panic!(
            "snapshot mismatch for {} ({} {})\n{}\n\
             actual {} bytes, expected {} bytes.\n\
             If the change is intentional, regenerate:\n  \
             RCA_UPDATE_SNAPSHOTS=1 cargo test --test snapshots",
            name,
            command,
            file,
            diff,
            actual.len(),
            expected.len()
        );
    }
}

#[test]
fn hello_lex() {
    check_snapshot("hello_lex.txt", "lex", "hello.rca");
}

#[test]
fn hello_parse() {
    check_snapshot("hello_parse.txt", "parse", "hello.rca");
}

#[test]
fn hello_ir() {
    check_snapshot("hello_ir.txt", "ir", "hello.rca");
}

#[test]
fn hello_ir_opt() {
    check_snapshot("hello_ir_opt.txt", "ir-opt", "hello.rca");
}

#[test]
fn full_parse() {
    check_snapshot("full_parse.txt", "parse", "full.rca");
}

#[test]
fn full_ir() {
    check_snapshot("full_ir.txt", "ir", "full.rca");
}

#[test]
fn opt_demo_ir_opt() {
    check_snapshot("opt_demo_ir_opt.txt", "ir-opt", "opt_demo.rca");
}
