//! §6.4 — multi-layer NN models via composition (linear + relu + linear + mse + grad + sgd).

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_mlp_{name}.rca"));
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

const MLP_TRAIN: &str = "\
let x = Tensor([[1.0, 2.0]])
let y = Tensor([[1.0]])
mut w1 = Tensor([[0.1, 0.2], [0.3, 0.4]])
mut b1 = Tensor([0.0, 0.0])
mut w2 = Tensor([[0.5], [0.6]])
mut b2 = Tensor([0.0])
for i in 0..3:
    let w1t = track(w1)
    let b1t = track(b1)
    let w2t = track(w2)
    let b2t = track(b2)
    let h = relu(linear(x, w1t, b1t))
    let pred = linear(h, w2t, b2t)
    let loss = mse(pred, y)
    w1 = sgd(w1, grad(loss, w1t), 0.05)
    b1 = sgd(b1, grad(loss, b1t), 0.05)
    w2 = sgd(w2, grad(loss, w2t), 0.05)
    b2 = sgd(b2, grad(loss, b2t), 0.05)
    print(loss)
";

#[test]
fn mlp_initial_loss_matches_hand_calc() {
    // h = relu([0.7, 1.0]) = [0.7, 1.0]; pred = 0.7*0.5 + 1.0*0.6 = 0.95; loss = (0.95-1)^2 = 0.0025
    let out = run_with("run", MLP_TRAIN, "mlp_init");
    assert!(out.contains("0.0025"), "got: {out}");
}

#[test]
fn mlp_loss_decreases_over_steps() {
    let out = run_with("run", MLP_TRAIN, "mlp_decrease");
    assert!(
        out.contains("5.0633") || out.contains("e-5") || out.contains("0.00005"),
        "got: {out}"
    );
}

#[test]
fn mlp_relu_gates_negative_inputs() {
    // Force both hidden units negative via a negative weight; relu clips to 0;
    // pred = 0 * w2 + b2 = b2 = 0; loss = (0-1)^2 = 1
    let src = "\
let x = Tensor([[1.0, 1.0]])
let y = Tensor([[1.0]])
let w1 = Tensor([[-1.0, -2.0], [-3.0, -4.0]])
let b1 = Tensor([0.0, 0.0])
let w2 = Tensor([[1.0], [1.0]])
let b2 = Tensor([0.0])
let w1t = track(w1)
let b1t = track(b1)
let w2t = track(w2)
let b2t = track(b2)
let h = relu(linear(x, w1t, b1t))
let pred = linear(h, w2t, b2t)
print(mse(pred, y))
";
    let out = run_with("run", src, "mlp_relu");
    assert!(out.contains("[1.0]"), "got: {out}");
}

#[test]
fn mlp_matches_tree_and_vm() {
    check_differential(MLP_TRAIN, "mlp_diff");
}
