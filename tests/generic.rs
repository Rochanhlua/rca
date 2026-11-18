use std::process::Command;

fn rcac(cmd: &str, file: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args([cmd, file])
        .output()
        .expect("failed to run rcac");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn generic_ok_runs() {
    let o = rcac("run", "examples/generic_ok.rca");
    assert_eq!(o.trim(), "10\nrca");
}

#[test]
fn generic_return_type_is_bound() {
    let o = rcac("run", "examples/generic_ret.rca");
    assert_eq!(o.trim(), "1\na");
}

#[test]
fn generic_mismatch_is_rejected() {
    let o = rcac("check", "examples/generic_bad.rca");
    assert!(o.contains("generic 'T' was Int, found String"), "got: {o}");
}
