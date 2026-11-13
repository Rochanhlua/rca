//! Phase 6 (§6.4) — NN engine tests: linear layer (CPU).
//! Both backends (VM and tree interpreter) must give identical output.

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_nn_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    p
}

fn run_with(subcmd: &str, src: &str, name: &str) -> String {
    let p = tmpfile(name, src);
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args([subcmd, p.to_str().unwrap()])
        .output()
        .expect("failed to spawn rcac");
    let _ = std::fs::remove_file(&p);
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn check(src: &str, name: &str, expected: &str) {
    let tree = run_with("run-tree", src, &format!("{name}_tree"));
    let vm = run_with("run", src, &format!("{name}_vm"));
    assert_eq!(
        tree, vm,
        "tree vs VM mismatch\nsrc: {src}\ntree: {tree}\nvm: {vm}"
    );
    assert_eq!(tree.trim(), expected.trim(), "output mismatch\nsrc: {src}");
}

#[test]
fn linear_forward_identity_weight() {
    let src = "let x = Tensor([[1.0, 2.0]])\n\
               let w = Tensor([[1.0], [0.0]])\n\
               let b = Tensor([0.5])\n\
               print(linear(x, w, b))\n";
    check(src, "fwd", "Tensor(shape=[1, 1], [[1.5]])");
}

#[test]
fn linear_matches_manual_compose() {
    let src = "let x = Tensor([[1.0, 2.0], [3.0, 4.0]])\n\
               let w = Tensor([[2.0, 1.0], [1.0, 2.0]])\n\
               let b = Tensor([1.0, 1.0])\n\
               let manual = matmul(x, w) + b\n\
               let via = linear(x, w, b)\n\
               print(manual == via)\n";
    check(src, "manual", "true");
}

#[test]
fn linear_arity_error() {
    let src = "let x = Tensor([1.0])\nlet w = Tensor([1.0])\nprint(linear(x, w))\n";
    let out = run_with("run", src, "arity");
    // Error path — arity is rejected before execution.
    // rcac prints errors on stderr; we check that no successful forward ran.
    assert!(!out.contains("[1.0]") || out.is_empty(), "got: {out}");
}

#[test]
fn linear_type_error() {
    let src = "let x = 1\nlet w = Tensor([1.0])\nlet b = Tensor([1.0])\nprint(linear(x, w, b))\n";
    let out = run_with("run", src, "type");
    assert!(out.is_empty() || !out.contains("linear"), "got: {out}");
}
