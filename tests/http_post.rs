//! §7 — HTTP POST loopback test.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

#[test]
fn http_post_loopback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap();
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        tx.send(req).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nposted")
            .unwrap();
    });

    let src = format!(
        "import http\nprint(http.post(\"http://127.0.0.1:{}/x\", \"hello\"))\n",
        port
    );
    let p = std::env::temp_dir().join("rca_http_post.rca");
    std::fs::write(&p, &src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rcac"))
        .args(["run", p.to_str().unwrap()])
        .output()
        .expect("spawn rcac");
    let _ = std::fs::remove_file(&p);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let req = rx.recv().unwrap();
    server.join().unwrap();

    assert!(req.starts_with("POST /x HTTP/1.1"), "req: {req}");
    assert!(req.contains("Content-Length: 5"), "req: {req}");
    assert!(req.contains("hello"), "req: {req}");
    assert!(stdout.contains("posted"), "stdout: {stdout}");
}
