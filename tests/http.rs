use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

#[test]
fn test_http_get() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_thread = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0; 1024];
        let _ = stream.read(&mut buf).unwrap();
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 12\r\nConnection: close\r\n\r\nHello, World!";
        stream.write_all(response.as_bytes()).unwrap();
    });

    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("http_test.rca");
    std::fs::write(
        &temp_file,
        format!(
            r#"
let body = http_get("http://127.0.0.1:{}/")
print(body)
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
    assert!(String::from_utf8_lossy(&out.stdout).contains("Hello, World!"));

    server_thread.join().unwrap();
    let _ = std::fs::remove_file(temp_file);
}
