# RCA Networking

Status: TCP, UDP, and plain HTTP/1.1 (GET/POST) are implemented as
builtins and as `import net` / `import http` members. Uses only
`std::net`, `std::io`, and `rustls` (for HTTPS).

## 1. TCP

| Builtin | Module form | Returns |
|---|---|---|
| `tcp_connect(host, port)` | `net.connect(host, port)` | Socket |
| `tcp_send(sock, data)` | `net.send(sock, data)` | Int (bytes written) |
| `tcp_recv(sock, max)` | `net.recv(sock, max)` | String (up to max bytes) |
| `tcp_close(sock)` | `net.close(sock)` | None |

`host` is a `String`, `port` an `Int`. Sockets are printed as `<socket>`.

## 2. UDP

| Builtin | Module form | Returns |
|---|---|---|
| `udp_bind(host, port)` | `net.udp_bind(host, port)` | UdpSocket |
| `udp_send(sock, host, port, data)` | `net.udp_send(...)` | Int (bytes sent) |
| `udp_recv(sock, max)` | `net.udp_recv(sock, max)` | Tuple `(data, src_host, src_port)` |
| `udp_close(sock)` | `net.udp_close(sock)` | None |

`udp_recv` returns a **3-tuple**: `(String data, String src_host, Int src_port)`.
Use `let (d, h, p) = udp_recv(s, 1024)` to destructure.

## 3. HTTP

| Builtin | Module form | Returns |
|---|---|---|
| `http_get(url)` | `http.get(url)` | String (response body) |
| `http_post(url, body)` | `http.post(url, body)` | String (response body) |

- URLs may start with `http://` or `https://`. HTTPS uses rustls with
  certificate verification on (`webpki-roots`).
- `http_post` sends `Content-Type: text/plain` with a correct
  `Content-Length` header.
- Response parsing strips the header block up to the first `\r\n\r\n`
  (or `\n\n`) and returns the body.
- HTTP/1.1, `Connection: close`.

## 4. Examples

TCP echo (see `tests/net.rs` for a loopback test):
    import net
    let s = net.connect("127.0.0.1", 8080)
    net.send(s, "ping")
    let reply = net.recv(s, 4)
    print(reply)
    net.close(s)

UDP loopback (see `examples/udp_loopback.rca`):
    import net
    let s = net.udp_bind("127.0.0.1", 38081)
    let n = net.udp_send(s, "127.0.0.1", 38081, "ping")
    let (d, h, p) = net.udp_recv(s, 64)
    print(d)
    net.udp_close(s)

HTTP:
    import http
    print(http.get("http://example.com/"))
    print(http.post("http://127.0.0.1:8080/echo", "hello"))

## 5. Constraints and limits
- **No timeouts** on TCP/UDP/HTTP operations. A non-responsive peer
  blocks the program.
- **No DNS-over-TLS**, no IPv6-conditional logic; address resolution uses
  `std::net` defaults.
- **`recv`/`udp_recv` are blocking.** They return when at least one byte
  arrives or the peer closes (TCP) or a datagram arrives (UDP).
- **No chunked transfer encoding** handling in HTTP; a chunked response
  is returned verbatim in the body.
- **No redirects, headers, cookies** — plain request/response body only.
- **No native codegen** for network ops; use `rcac run`.

## 6. TLS (HTTPS)
`https://` URLs work via `rustls 0.23` with the `aws-lc-rs` backend and
`webpki-roots` trust anchors. Certificate verification is enabled. SNI
is set from the URL host. Default port 443.

`http://` is unchanged. `pkg install` accepts both schemes.

## 7. Tests
- `tests/net.rs` — TCP loopback (server on `TcpListener`, client via `rcac`).
- `tests/udp.rs` — bind/close, loopback send/recv (port 38081 to avoid
  Windows ephemeral range), arity, unknown item.
- `tests/http.rs` — loopback HTTP GET.
- `tests/http_post.rs` — loopback HTTP POST, verifies method line,
  Content-Length, body, response.
