//! §6.4 — conv2d builtin (CPU, valid padding, stride 1). VM and tree must agree.

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_cv_{name}.rca"));
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

const ID_X: &str = "let x = Tensor([[[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]]])\n";

#[test]
fn conv2d_identity_kernel() {
    let src = format!("{ID_X}print(conv2d(x, Tensor([[[[1.0]]]])))\n");
    let out = run_with("run", &src, "cv_id");
    assert!(out.contains("shape=[1, 1, 3, 3]"), "got: {out}");
    assert!(out.contains("1.0") && out.contains("9.0"), "got: {out}");
}

#[test]
fn conv2d_bottom_right_picker() {
    let src = format!("{ID_X}print(conv2d(x, Tensor([[[[0.0, 0.0], [0.0, 1.0]]]])))\n");
    let out = run_with("run", &src, "cv_pick");
    assert!(out.contains("shape=[1, 1, 2, 2]"), "got: {out}");
    assert!(out.contains("[[[[5.0, 6.0], [8.0, 9.0]]]]"), "got: {out}");
}

#[test]
fn conv2d_2x2_sum_kernel() {
    let src = format!("{ID_X}print(conv2d(x, Tensor([[[[1.0, 1.0], [1.0, 1.0]]]])))\n");
    let out = run_with("run", &src, "cv_sum");
    assert!(
        out.contains("[[[[12.0, 16.0], [24.0, 28.0]]]]"),
        "got: {out}"
    );
}

#[test]
fn conv2d_rank3_input_error() {
    let out = run_with(
        "run",
        "print(conv2d(Tensor([[1.0]]), Tensor([[[[1.0]]]])))\n",
        "cv_rank",
    );
    assert!(out.to_lowercase().contains("rank 4"), "got: {out}");
}

#[test]
fn conv2d_channel_mismatch() {
    let out = run_with(
        "run",
        "print(conv2d(Tensor([[[[1.0]]]]), Tensor([[[[1.0]], [[2.0]]]])))\n",
        "cv_ch",
    );
    let all = out.to_lowercase();
    assert!(all.contains("channel mismatch"), "got: {out}");
}

#[test]
fn conv2d_kernel_larger_error() {
    let out = run_with(
        "run",
        "print(conv2d(Tensor([[[[1.0]]]]), Tensor([[[[1.0, 2.0], [3.0, 4.0]]]])))\n",
        "cv_big",
    );
    assert!(out.to_lowercase().contains("kernel larger"), "got: {out}");
}

#[test]
fn conv2d_arity_error() {
    let out = run_with("run", "print(conv2d(Tensor([[[[1.0]]]])))\n", "cv_arity");
    assert!(
        out.to_lowercase().contains("exactly 2 arguments"),
        "got: {out}"
    );
}

#[test]
fn conv2d_matches_tree_and_vm() {
    let src = format!("{ID_X}print(conv2d(x, Tensor([[[[1.0]]]])))\n");
    check_differential(&src, "cv_diff1");
    let src2 = format!("{ID_X}print(conv2d(x, Tensor([[[[1.0, 1.0], [1.0, 1.0]]]])))\n");
    check_differential(&src2, "cv_diff2");
}
