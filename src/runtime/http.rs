//! Minimal HTTP/1.1 client implemented in pure Rust.
use std::io::{Read, Write};
use std::net::TcpStream;

pub fn http_get(url: &str) -> Result<String, String> {
    let url = url.trim();
    let (host, port, path) = parse_url(url)?;
    let addr = format!("{}:{}", host, port);
    let mut stream =
        TcpStream::connect(&addr).map_err(|e| format!("http_get connect failed: {}", e))?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: RCA/0.1\r\n\r\n",
        path, host
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("http_get write failed: {}", e))?;
    let mut resp = String::new();
    stream
        .read_to_string(&mut resp)
        .map_err(|e| format!("http_get read failed: {}", e))?;
    if let Some(pos) = resp.find("\r\n\r\n") {
        Ok(resp[pos + 4..].to_string())
    } else if let Some(pos) = resp.find("\n\n") {
        Ok(resp[pos + 2..].to_string())
    } else {
        Ok(resp)
    }
}

fn parse_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = if let Some(r) = url.strip_prefix("http://") {
        r
    } else {
        return Err("http_get needs an http:// URL".into());
    };
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.find(':') {
        Some(i) => {
            let p = host_port[i + 1..]
                .parse::<u16>()
                .map_err(|_| "invalid port in URL")?;
            (&host_port[..i], p)
        }
        None => (host_port, 80),
    };
    if host.is_empty() {
        return Err("empty host in URL".into());
    }
    Ok((host.to_string(), port, path.to_string()))
}
