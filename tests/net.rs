use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

#[test]
fn test_tcp_builtins() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();

    let server_thread = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0; 1024];
        let n = stream.read(&mut buf).unwrap();
        tx.send(String::from_utf8_lossy(&buf[..n]).to_string())
            .unwrap();
        stream.write_all(b"pong").unwrap();
    });

    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("net_test.rca");
    std::fs::write(
        &temp_file,
        format!(
            r#"
let sock = tcp_connect("127.0.0.1", {})
let sent = tcp_send(sock, "ping")
let reply = tcp_recv(sock, 4)
print(reply)
tcp_close(sock)
"#,
            port
        ),
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_rca"))
        .arg("run")
        .arg(&temp_file)
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("pong"));
    assert_eq!(rx.recv().unwrap(), "ping");

    server_thread.join().unwrap();
    let _ = std::fs::remove_file(temp_file);
}
