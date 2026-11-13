//! §6.4 — sgd() optimizer builtin (CPU). VM and tree must agree.

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_sgd_{name}.rca"));
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
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

fn check_differential(src: &str, name: &str) {
    let tree = run_with("run-tree", src, &format!("{name}_tree"));
    let vm = run_with("run", src, &format!("{name}_vm"));
    assert_eq!(
        tree, vm,
        "tree vs VM mismatch\nsrc: {src}\ntree: {tree}\nvm: {vm}"
    );
}

#[test]
fn sgd_float_lr() {
    let src = "print(sgd(Tensor([1.0, 2.0]), Tensor([0.5, 1.0]), 0.1))\n";
    let out = run_with("run", src, "sgd_f");
    assert!(out.contains("[0.95, 1.9]"), "got: {out}");
}

#[test]
fn sgd_int_lr_coerced() {
    let src = "print(sgd(Tensor([1.0, 2.0]), Tensor([0.5, 1.0]), 1))\n";
    let out = run_with("run", src, "sgd_i");
    assert!(out.contains("[0.5, 1.0]"), "got: {out}");
}

#[test]
fn sgd_zero_grad_unchanged() {
    let src = "print(sgd(Tensor([[1.0, 2.0]]), Tensor([[0.0, 0.0]]), 0.5))\n";
    let out = run_with("run", src, "sgd_zero");
    assert!(out.contains("[[1.0, 2.0]]"), "got: {out}");
}

#[test]
fn sgd_with_autodiff_grad() {
    let src = "let w = track(Tensor([2.0]))\nlet loss = w * w\nlet g = grad(loss, w)\nprint(sgd(Tensor([2.0]), g, 0.1))\n";
    let out = run_with("run", src, "sgd_grad");
    assert!(out.contains("[1.6]"), "got: {out}");
}

#[test]
fn sgd_shape_mismatch() {
    let out = run_with(
        "run",
        "print(sgd(Tensor([1.0, 2.0]), Tensor([1.0]), 0.1))\n",
        "sgd_shape",
    );
    assert!(
        out.to_lowercase().contains("shapes do not match"),
        "got: {out}"
    );
}

#[test]
fn sgd_arity_error() {
    let out = run_with(
        "run",
        "print(sgd(Tensor([1.0]), Tensor([1.0])))\n",
        "sgd_arity",
    );
    assert!(
        out.to_lowercase().contains("exactly 3 arguments"),
        "got: {out}"
    );
}

#[test]
fn sgd_bad_lr_type() {
    let out = run_with(
        "run",
        "print(sgd(Tensor([1.0]), Tensor([1.0]), \"fast\"))\n",
        "sgd_lr",
    );
    assert!(out.to_lowercase().contains("needs float lr"), "got: {out}");
}

#[test]
fn sgd_matches_tree_and_vm() {
    check_differential(
        "print(sgd(Tensor([1.0, 2.0, 3.0]), Tensor([0.1, 0.2, 0.3]), 0.5))\n",
        "sgd_diff1",
    );
    check_differential(
        "let w = track(Tensor([3.0]))\nlet loss = w * w\nlet g = grad(loss, w)\nprint(sgd(Tensor([3.0]), g, 0.01))\n",
        "sgd_diff2",
    );
}
