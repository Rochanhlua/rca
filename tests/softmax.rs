//! §6.4 — softmax builtin (CPU). VM and tree must agree.

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_sm_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    p
}

/// Returns combined stdout+stderr.
fn run_with(subcmd: &str, src: &str, name: &str) -> String {
    let p = tmpfile(name, src);
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args([subcmd, p.to_str().unwrap()])
        .output()
        .expect("failed to spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

/// Compare tree vs VM output for a valid program.
fn check_differential(src: &str, name: &str) {
    let tree = run_with("run-tree", src, &format!("{name}_tree"));
    let vm = run_with("run", src, &format!("{name}_vm"));
    assert_eq!(
        tree, vm,
        "tree vs VM mismatch\nsrc: {src}\ntree: {tree}\nvm: {vm}"
    );
}

#[test]
fn softmax_1d_sums_to_one() {
    let src = "let x = Tensor([1.0, 2.0, 3.0])\nprint(softmax(x))\n";
    let out = run_with("run", src, "sm_1d");
    assert!(out.contains("0.090030"), "got: {out}");
    assert!(out.contains("0.665240"), "got: {out}");
}

#[test]
fn softmax_2d_rowwise() {
    let src = "let m = Tensor([[1.0, 2.0], [3.0, 4.0]])\nprint(softmax(m))\n";
    let out = run_with("run", src, "sm_2d");
    assert!(out.contains("0.268941"), "got: {out}");
    assert!(out.contains("0.731058"), "got: {out}");
}

#[test]
fn softmax_numerically_stable() {
    let src = "print(softmax(Tensor([1000.0, 1001.0])))\n";
    let out = run_with("run", src, "sm_stable");
    assert!(out.contains("0.268941"), "got: {out}");
    assert!(!out.contains("NaN"), "got: {out}");
    assert!(!out.to_lowercase().contains("inf"), "got: {out}");
}

#[test]
fn softmax_type_error() {
    let out = run_with("run", "print(softmax(1))\n", "sm_bad");
    let all = out.to_lowercase();
    assert!(all.contains("needs a tensor"), "got: {out}");
}

#[test]
fn softmax_arity_error() {
    let out = run_with("run", "print(softmax(Tensor([1.0]), 2))\n", "sm_arity");
    let all = out.to_lowercase();
    assert!(all.contains("exactly 1 argument"), "got: {out}");
}

#[test]
fn softmax_matches_tree_and_vm() {
    check_differential("print(softmax(Tensor([1.0, 2.0, 3.0])))\n", "sm_diff1");
    check_differential(
        "print(softmax(Tensor([[1.0, 2.0], [3.0, 4.0]])))\n",
        "sm_diff2",
    );
    check_differential("print(softmax(Tensor([1000.0, 1001.0])))\n", "sm_diff3");
}
