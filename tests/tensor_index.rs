use std::process::Command;

fn run(cmd: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args([cmd, "examples/tensor_index.rca"])
        .output()
        .expect("failed to run rcac");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn comma_index_reads_elements() {
    assert_eq!(run("run").trim(), "2.0\n3.0\n4.0");
}

#[test]
fn vm_and_tree_walker_agree() {
    assert_eq!(run("run"), run("run-tree"));
}
