//! §12 Phase 7 dev tool #1 — `rcac lint`.

use std::process::Command;

fn run(src: &str, name: &str) -> String {
    let p = std::env::temp_dir().join(format!("rca_lint_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["lint", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn unused_variable_warns() {
    let out = run("let unused = 42\nprint(1)\n", "unused");
    assert!(out.contains("unused variable 'unused'"), "got: {out}");
}

#[test]
fn used_variable_is_clean() {
    let out = run("let x = 5\nprint(x)\n", "used");
    assert!(!out.contains("unused variable"), "got: {out}");
    assert!(out.contains("0 warning"), "got: {out}");
}

#[test]
fn unreachable_after_return_warns() {
    let out = run(
        "fn f() -> Int:\n    return 1\n    print(2)\nprint(f())\n",
        "unreach",
    );
    assert!(out.contains("unreachable statement"), "got: {out}");
}

#[test]
fn for_loop_var_unused_warns() {
    let out = run("for i in 0..3:\n    print(0)\n", "forvar");
    assert!(out.contains("unused variable 'i'"), "got: {out}");
}

#[test]
fn for_loop_var_used_is_clean() {
    let out = run("for i in 0..3:\n    print(i)\n", "forused");
    assert!(!out.contains("unused variable 'i'"), "got: {out}");
}

#[test]
fn let_tuple_unused_names_warn() {
    let out = run("let (a, b) = (1, 2)\nprint(a)\n", "tuple");
    assert!(out.contains("unused variable 'b'"), "got: {out}");
    assert!(!out.contains("unused variable 'a'"), "got: {out}");
}
