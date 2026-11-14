//! §7 — HMAC-SHA256 builtin (RFC 4231 known-answer vectors).

use std::path::PathBuf;
use std::process::Command;

fn run(src: &str, name: &str) -> String {
    let p: PathBuf = std::env::temp_dir().join(format!("rca_hmac_{name}.rca"));
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
fn hmac_rfc4231_case2() {
    let out = run(
        "import crypto\nprint(crypto.hmac_sha256(\"Jefe\", \"what do ya want for nothing?\"))\n",
        "case2",
    );
    assert_eq!(
        out.trim(),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
        "got: {out}"
    );
}

#[test]
fn hmac_key_quick_brown() {
    let out = run(
        "import crypto\nprint(crypto.hmac_sha256(\"key\", \"The quick brown fox jumps over the lazy dog\"))\n",
        "fox",
    );
    assert_eq!(
        out.trim(),
        "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8",
        "got: {out}"
    );
}

#[test]
fn hmac_empty_key_and_message() {
    // RFC 4231 does not define empty/empty; assert deterministic 64-hex output.
    let out = run(
        "import crypto\nprint(crypto.hmac_sha256(\"\", \"\"))\n",
        "empty",
    );
    let t = out.trim();
    assert_eq!(t.len(), 64, "expected 64 hex chars, got: {t}");
    assert!(t.chars().all(|c| c.is_ascii_hexdigit()), "got: {t}");
}

#[test]
fn hmac_type_error() {
    let out = run(
        "import crypto\nprint(crypto.hmac_sha256(123, \"msg\"))\n",
        "type",
    );
    assert!(out.to_lowercase().contains("needs string"), "got: {out}");
}

#[test]
fn hmac_arity_error() {
    let out = run("import crypto\nprint(crypto.hmac_sha256(\"k\"))\n", "arity");
    assert!(out.to_lowercase().contains("exactly 2"), "got: {out}");
}
