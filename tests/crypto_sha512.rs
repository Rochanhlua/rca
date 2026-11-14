//! §7 — SHA-512 builtin (crypto module).

use std::path::PathBuf;
use std::process::Command;

fn run(src: &str, name: &str) -> String {
    let p: PathBuf = std::env::temp_dir().join(format!("rca_sha512_{name}.rca"));
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
fn sha512_empty_vector() {
    let out = run("import crypto\nprint(crypto.sha512(\"\"))\n", "empty");
    assert_eq!(
        out.trim(),
        "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
        "got: {out}"
    );
}

#[test]
fn sha512_abc_vector() {
    let out = run("import crypto\nprint(crypto.sha512(\"abc\"))\n", "abc");
    assert_eq!(
        out.trim(),
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        "got: {out}"
    );
}

#[test]
fn sha512_type_error() {
    let out = run("import crypto\nprint(crypto.sha512(123))\n", "type");
    assert!(out.to_lowercase().contains("needs a string"), "got: {out}");
}
