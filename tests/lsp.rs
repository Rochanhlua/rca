//! §12 Phase 7 dev tool #5 — `rcac lsp` (minimal).

use std::io::Write;
use std::process::{Command, Stdio};

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

fn run_lsp(input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rcac lsp");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(input.as_bytes()).ok();
    }
    let out = child.wait_with_output().expect("wait");
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn lsp_initialize_responds() {
    let req = frame(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#);
    let exit = frame(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    let out = run_lsp(&format!("{}{}", req, exit));
    assert!(out.contains("Content-Length:"), "got: {out}");
    assert!(out.contains("\"jsonrpc\":\"2.0\""), "got: {out}");
    assert!(out.contains("\"result\""), "got: {out}");
}

#[test]
fn lsp_shutdown_responds() {
    let shutdown = frame(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#);
    let exit = frame(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    let out = run_lsp(&format!("{}{}", shutdown, exit));
    assert!(out.contains("\"id\":2"), "got: {out}");
    assert!(out.contains("\"result\""), "got: {out}");
}

#[test]
fn lsp_publishes_diagnostics_on_open() {
    let didopen = frame(
        r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///a.rca","text":"let x = "}}}"#,
    );
    let exit = frame(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    let out = run_lsp(&format!("{}{}", didopen, exit));
    assert!(
        out.contains("textDocument/publishDiagnostics"),
        "got: {out}"
    );
    assert!(out.contains("file:///a.rca"), "got: {out}");
}

#[test]
fn lsp_clean_file_no_diagnostics() {
    let didopen = frame(
        r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///b.rca","text":"let x = 5\nprint(x)\n"}}}"#,
    );
    let exit = frame(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    let out = run_lsp(&format!("{}{}", didopen, exit));
    assert!(
        out.contains("textDocument/publishDiagnostics"),
        "got: {out}"
    );
    assert!(out.contains("\"diagnostics\":[]"), "got: {out}");
}

#[test]
fn lsp_exit_ends_process() {
    let exit = frame(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    let out = run_lsp(&exit);
    // No response for a notification; output is empty or whitespace only.
    assert!(!out.contains("\"error\""), "got: {out}");
}
