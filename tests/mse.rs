//! §6.4 — mse() loss builtin (CPU). VM and tree must agree.

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_mse_{name}.rca"));
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
fn mse_identical_is_zero() {
    let src = "print(mse(Tensor([1.0, 2.0, 3.0]), Tensor([1.0, 2.0, 3.0])))\n";
    let out = run_with("run", src, "mse_zero");
    assert!(out.contains("shape=[1], [0.0]"), "got: {out}");
}

#[test]
fn mse_simple_case() {
    let src = "print(mse(Tensor([1.0, 2.0]), Tensor([3.0, 4.0])))\n";
    let out = run_with("run", src, "mse_4");
    assert!(out.contains("shape=[1], [4.0]"), "got: {out}");
}

#[test]
fn mse_offset_by_one() {
    let src = "print(mse(Tensor([0.0, 0.0]), Tensor([1.0, 1.0])))\n";
    let out = run_with("run", src, "mse_1");
    assert!(out.contains("shape=[1], [1.0]"), "got: {out}");
}

#[test]
fn mse_single_element() {
    let src = "print(mse(Tensor([2.0]), Tensor([5.0])))\n";
    let out = run_with("run", src, "mse_9");
    assert!(out.contains("shape=[1], [9.0]"), "got: {out}");
}

#[test]
fn mse_rank2_tensors() {
    let src = "print(mse(Tensor([[1.0, 2.0], [3.0, 4.0]]), Tensor([[1.0, 2.0], [3.0, 4.0]])))\n";
    let out = run_with("run", src, "mse_r2");
    assert!(out.contains("shape=[1], [0.0]"), "got: {out}");
}

#[test]
fn mse_shape_mismatch() {
    let out = run_with(
        "run",
        "print(mse(Tensor([1.0, 2.0]), Tensor([1.0])))\n",
        "mse_shape",
    );
    assert!(
        out.to_lowercase().contains("shapes do not match"),
        "got: {out}"
    );
}

#[test]
fn mse_arity_error() {
    let out = run_with("run", "print(mse(Tensor([1.0])))\n", "mse_arity");
    assert!(
        out.to_lowercase().contains("exactly 2 arguments"),
        "got: {out}"
    );
}

#[test]
fn mse_non_tensor_error() {
    let out = run_with("run", "print(mse(1.0, Tensor([1.0])))\n", "mse_ty");
    assert!(out.to_lowercase().contains("needs tensor"), "got: {out}");
}

#[test]
fn mse_matches_tree_and_vm() {
    check_differential(
        "print(mse(Tensor([1.0, 2.0, 3.0]), Tensor([4.0, 5.0, 6.0])))\n",
        "mse_diff1",
    );
    check_differential(
        "print(mse(Tensor([[1.0, 2.0], [3.0, 4.0]]), Tensor([[0.0, 1.0], [2.0, 3.0]])))\n",
        "mse_diff2",
    );
}
