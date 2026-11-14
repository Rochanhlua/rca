//! §7 — HTTPS GET via rustls (live example.com).

use std::process::Command;

fn run(src: &str, name: &str) -> String {
    let p = std::env::temp_dir().join(format!("rca_https_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
#[ignore = "live network: enable with --ignored"]
fn https_get_example_com() {
    let out = run(
        "import http\nlet body = http.get(\"https://example.com/\")\nprint(len(body))\nprint(body)\n",
        "live",
    );
    assert!(!out.contains("runtime error"), "got: {out}");
    assert!(out.contains("Example Domain"), "got: {out}");
}

#[test]
fn https_bad_host_errors() {
    let out = run(
        "import http\nprint(http.get(\"https://nonexistent.invalid/\"))\n",
        "bad",
    );
    assert!(
        out.to_lowercase().contains("https: connect failed")
            || out.to_lowercase().contains("error"),
        "got: {out}"
    );
}

#[test]
fn https_arity_error() {
    let out = run("import http\nhttp.get()\n", "arity");
    assert!(out.to_lowercase().contains("exactly 1"), "got: {out}");
}
