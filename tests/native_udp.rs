use std::process::Command;

#[test]
fn native_udp_loopback() {
    let dir = std::env::temp_dir().join("rca_native_udp");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t_udp.rca");
    std::fs::write(
        &src,
        "import net\nlet s = net.udp_bind(\"127.0.0.1\", 38091)\nlet n = net.udp_send(s, \"127.0.0.1\", 38091, \"ping\")\nprint(n)\nlet (d, h, p) = net.udp_recv(s, 64)\nprint(d)\nprint(h)\nprint(p)\nnet.udp_close(s)\n",
    )
    .unwrap();
    let rcac = env!("CARGO_BIN_EXE_rcac");
    let b = Command::new(rcac).arg("build").arg(&src).output().unwrap();
    assert!(b.status.success(), "{}", String::from_utf8_lossy(&b.stderr));
    let out = Command::new(dir.join(if cfg!(windows) { "t_udp.exe" } else { "t_udp" }))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "4\nping\n127.0.0.1\n38091\n"
    );
}
