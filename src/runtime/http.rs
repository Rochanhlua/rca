//! Minimal HTTP/1.1 client implemented in pure Rust.
use std::io::{Read, Write};
use std::net::TcpStream;

pub fn http_get(url: &str) -> Result<String, String> {
    let url = url.trim();
    if url.starts_with("https://") {
        return https_request(url, "GET", None);
    }
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

/// HTTP POST. Sends `body` as text/plain; returns the response body.
pub fn http_post(url: &str, body: &str) -> Result<String, String> {
    let url = url.trim();
    if url.starts_with("https://") {
        return https_request(url, "POST", Some(body));
    }
    let (host, port, path) = parse_url(url)?;
    let addr = format!("{}:{}", host, port);
    let mut stream =
        TcpStream::connect(&addr).map_err(|e| format!("http_post connect failed: {}", e))?;
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: RCA/0.1\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        path,
        host,
        body.len(),
        body
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("http_post write failed: {}", e))?;
    let mut resp = String::new();
    stream
        .read_to_string(&mut resp)
        .map_err(|e| format!("http_post read failed: {}", e))?;
    if let Some(pos) = resp.find("\r\n\r\n") {
        Ok(resp[pos + 4..].to_string())
    } else if let Some(pos) = resp.find("\n\n") {
        Ok(resp[pos + 2..].to_string())
    } else {
        Ok(resp)
    }
}

/// HTTPS GET/POST via rustls.
fn https_request(url: &str, method: &str, body: Option<&str>) -> Result<String, String> {
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
    use std::io::{Read, Write};
    use std::sync::Arc;

    let (host, port, path) = parse_https_url(url)?;

    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let server_name = ServerName::try_from(host.clone())
        .map_err(|e| format!("https: invalid server name: {}", e))?;
    let conn = ClientConnection::new(Arc::new(config), server_name)
        .map_err(|e| format!("https: TLS setup failed: {}", e))?;
    let tcp = TcpStream::connect(format!("{}:{}", host, port))
        .map_err(|e| format!("https: connect failed: {}", e))?;
    let mut tls = StreamOwned::new(conn, tcp);

    let req = match body {
        Some(b) => format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: RCA/0.1\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            method,
            path,
            host,
            b.len(),
            b
        ),
        None => format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: RCA/0.1\r\n\r\n",
            method, path, host
        ),
    };
    tls.write_all(req.as_bytes())
        .map_err(|e| format!("https: write failed: {}", e))?;
    let mut resp = String::new();
    tls.read_to_string(&mut resp)
        .map_err(|e| format!("https: read failed: {}", e))?;
    if let Some(pos) = resp.find("\r\n\r\n") {
        Ok(resp[pos + 4..].to_string())
    } else if let Some(pos) = resp.find("\n\n") {
        Ok(resp[pos + 2..].to_string())
    } else {
        Ok(resp)
    }
}

fn parse_https_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "https_request needs an https:// URL".to_string())?;
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.find(':') {
        Some(i) => {
            let p = host_port[i + 1..]
                .parse::<u16>()
                .map_err(|_| "https: invalid port in URL".to_string())?;
            (host_port[..i].to_string(), p)
        }
        None => (host_port.to_string(), 443u16),
    };
    if host.is_empty() {
        return Err("https: empty host in URL".to_string());
    }
    Ok((host, port, path.to_string()))
}
