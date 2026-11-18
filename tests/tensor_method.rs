use std::process::Command;

fn run(cmd: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args([cmd, "examples/tensor_method.rca"])
        .output()
        .expect("failed to run rcac");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn method_call_equals_function_call() {
    let lines: Vec<String> = run("run").lines().map(String::from).collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0], lines[1]);
    assert_eq!(
        lines[0],
        "Tensor(shape=[2, 2], [[19.0, 22.0], [43.0, 50.0]])"
    );
    assert_eq!(lines[3], "10.0");
}

#[test]
fn vm_and_tree_walker_agree() {
    assert_eq!(run("run"), run("run-tree"));
}
