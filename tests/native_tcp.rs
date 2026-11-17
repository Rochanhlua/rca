use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

#[test]
fn native_tcp_echo() {
    let listener = TcpListener::bind("127.0.0.1:38090").unwrap();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ping");
        s.write_all(b"pong").unwrap();
    });

    let dir = std::env::temp_dir().join("rca_native_tcp");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("t_echo.rca");
    std::fs::write(
        &src,
        "import net\nlet s = net.connect(\"127.0.0.1\", 38090)\nlet n = net.send(s, \"ping\")\nprint(n)\nlet r = net.recv(s, 16)\nprint(r)\nnet.close(s)\n",
    )
    .unwrap();

    let rcac = env!("CARGO_BIN_EXE_rcac");
    let b = Command::new(rcac).arg("build").arg(&src).output().unwrap();
    assert!(b.status.success(), "{}", String::from_utf8_lossy(&b.stderr));

    let out = Command::new(dir.join(if cfg!(windows) {
        "t_echo.exe"
    } else {
        "t_echo"
    }))
    .output()
    .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "4\npong\n");
    server.join().unwrap();
}
