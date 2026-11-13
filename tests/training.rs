//! §6.4 — end-to-end training pipeline (track + linear + mse + grad + sgd).

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_train_{name}.rca"));
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

/// One SGD step on linear regression: w=0.1, b=0; x=[1,2], y=[3].
/// pred = 0.1*1 + 0.1*2 + 0 = 0.3; loss = (0.3-3)^2 = 7.29
const ONE_STEP: &str = "\
let x = Tensor([[1.0, 2.0]])
let y = Tensor([[3.0]])
let w = Tensor([[0.1], [0.1]])
let b = Tensor([0.0])
let wt = track(w)
let bt = track(b)
let pred = linear(x, wt, bt)
let loss = mse(pred, y)
print(loss)
";

#[test]
fn mse_on_tracked_linear() {
    let out = run_with("run", ONE_STEP, "tracked_loss");
    assert!(out.contains("7.29"), "got: {out}");
}

#[test]
fn training_loop_decreases_loss() {
    let src = "\
let x = Tensor([[1.0, 2.0]])
let y = Tensor([[3.0]])
mut w = Tensor([[0.1], [0.1]])
mut b = Tensor([0.0])
for i in 0..5:
    let wt = track(w)
    let bt = track(b)
    let pred = linear(x, wt, bt)
    let loss = mse(pred, y)
    let gw = grad(loss, wt)
    let gb = grad(loss, bt)
    w = sgd(w, gw, 0.05)
    b = sgd(b, gb, 0.05)
    print(loss)
";
    let out = run_with("run", src, "train_loop");
    // First loss 7.29, last loss < 0.01
    assert!(out.contains("7.29"), "got: {out}");
    let tail: Vec<&str> = out.lines().collect();
    assert!(tail.len() >= 5, "expected 5 loss lines, got: {out}");
    assert!(
        out.contains("0.0047") || out.contains("0.004"),
        "loss did not shrink: {out}"
    );
}

#[test]
fn training_loop_updates_weights() {
    let src = "\
let x = Tensor([[1.0, 2.0]])
let y = Tensor([[3.0]])
mut w = Tensor([[0.1], [0.1]])
mut b = Tensor([0.0])
for i in 0..3:
    let wt = track(w)
    let bt = track(b)
    let pred = linear(x, wt, bt)
    let loss = mse(pred, y)
    w = sgd(w, grad(loss, wt), 0.05)
    b = sgd(b, grad(loss, bt), 0.05)
print(w)
print(b)
";
    let out = run_with("run", src, "train_upd");
    // w and b must have moved away from initial 0.1 / 0.0
    assert!(!out.contains("[[0.1], [0.1]]"), "w did not update: {out}");
    assert!(!out.contains("shape=[1], [0.0]"), "b did not update: {out}");
}

#[test]
fn training_pipeline_matches_tree_and_vm() {
    check_differential(ONE_STEP, "train_diff");
    let loop_src = "\
let x = Tensor([[1.0, 2.0]])
let y = Tensor([[3.0]])
mut w = Tensor([[0.1], [0.1]])
mut b = Tensor([0.0])
for i in 0..3:
    let wt = track(w)
    let bt = track(b)
    let pred = linear(x, wt, bt)
    let loss = mse(pred, y)
    w = sgd(w, grad(loss, wt), 0.05)
    b = sgd(b, grad(loss, bt), 0.05)
print(w)
print(b)
";
    check_differential(loop_src, "train_diff_loop");
}
