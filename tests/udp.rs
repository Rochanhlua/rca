//! §7 — UDP builtins (net module).

use std::path::PathBuf;
use std::process::Command;

fn tmpfile(name: &str, src: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("rca_udp_{name}.rca"));
    std::fs::write(&p, src).unwrap();
    p
}

fn run(src: &str, name: &str) -> String {
    let p = tmpfile(name, src);
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", p.to_str().unwrap()])
        .output()
        .expect("failed to spawn rcac");
    let _ = std::fs::remove_file(&p);
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    s
}

#[test]
fn udp_bind_close() {
    let src = "import net\nlet s = net.udp_bind(\"127.0.0.1\", 0)\nprint(s)\nnet.udp_close(s)\nprint(\"done\")\n";
    let out = run(src, "bind");
    assert!(out.contains("<udp_socket>"), "got: {out}");
    assert!(out.contains("done"), "got: {out}");
}

#[test]
fn udp_loopback_send_recv() {
    let src = "import net\nlet s = net.udp_bind(\"127.0.0.1\", 38081)\nlet n = net.udp_send(s, \"127.0.0.1\", 38081, \"ping\")\nprint(n)\nlet reply = net.udp_recv(s, 64)\nprint(reply)\nnet.udp_close(s)\n";
    let out = run(src, "loop");
    assert!(out.contains("4"), "got: {out}");
    assert!(out.contains("ping"), "got: {out}");
    assert!(out.contains("127.0.0.1"), "got: {out}");
    assert!(out.contains("38081"), "got: {out}");
}

#[test]
fn udp_bind_arity_error() {
    let src = "import net\nprint(net.udp_bind(\"127.0.0.1\"))\n";
    let out = run(src, "arity");
    assert!(out.to_lowercase().contains("exactly 2"), "got: {out}");
}

#[test]
fn udp_unknown_item_errors() {
    let src = "import net\nnet.udp_nosuch(1)\n";
    let out = run(src, "baditem");
    let all = out.to_lowercase();
    assert!(
        all.contains("no item") || all.contains("undefined"),
        "got: {out}"
    );
}
