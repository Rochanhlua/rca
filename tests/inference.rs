//! §6.4 — inference pipeline (forward only, no autodiff).

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_inf_{name}.rca"));
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

const PREDICT_FN: &str = "\
fn predict(x: Tensor, w: Tensor, b: Tensor) -> Tensor:
    return linear(x, w, b)
";

#[test]
fn predict_forward_only() {
    let src = format!(
        "{PREDICT_FN}let x = Tensor([[1.0, 2.0]])\nlet w = Tensor([[0.1], [0.2]])\nlet b = Tensor([0.0])\nprint(predict(x, w, b))\n"
    );
    let out = run_with("run", &src, "pred_fwd");
    assert!(out.contains("[[0.5]]"), "got: {out}");
}

#[test]
fn train_then_inference() {
    let src = "\
fn predict(x: Tensor, w: Tensor, b: Tensor) -> Tensor:
    return linear(x, w, b)
fn train_weights() -> Tensor:
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
    return w
let w = train_weights()
let b = Tensor([0.0])
print(predict(Tensor([[2.0, 3.0]]), w, b))
";
    let out = run_with("run", src, "train_inf");
    assert!(out.contains("3.869"), "got: {out}");
}

#[test]
fn inference_matches_tree_and_vm() {
    let src = format!(
        "{PREDICT_FN}let x = Tensor([[1.0, 2.0]])\nlet w = Tensor([[0.1], [0.2]])\nlet b = Tensor([0.0])\nprint(predict(x, w, b))\n"
    );
    check_differential(&src, "inf_diff");
}
