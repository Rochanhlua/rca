//! C ABI helpers for native codegen.
//!
//! These `extern "C"` functions are called from Cranelift-generated code and
//! linked into the final executable via `librca.a`.
//!
//! String convention: RCA native strings are null-terminated C strings. FFI
//! helpers accept and return that same layout.

use std::ffi::CStr;
use std::os::raw::c_char;

/// HTTPS/HTTP GET. `url_ptr` is a null-terminated C string. On success
/// returns a null-terminated C string allocated with the Rust allocator
/// (caller owns; use `rca_free`). On failure prints to stderr and returns NULL.
///
/// # Safety
/// `url_ptr` must be a valid, non-null, null-terminated C string that stays
/// valid for the duration of the call. The returned pointer must be freed
/// with `rca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_http_get(url_ptr: *const c_char) -> *mut c_char {
    if url_ptr.is_null() {
        eprintln!("rca_http_get: null url");
        return std::ptr::null_mut();
    }
    let url = match unsafe { CStr::from_ptr(url_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("rca_http_get: url is not valid UTF-8");
            return std::ptr::null_mut();
        }
    };
    match crate::runtime::http::http_get(url) {
        Ok(body) => {
            let mut bytes = body.into_bytes();
            bytes.push(0);
            let len = bytes.len();
            let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
            let ptr = unsafe { std::alloc::alloc(layout) };
            if ptr.is_null() {
                eprintln!("rca_http_get: alloc failed");
                return std::ptr::null_mut();
            }
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len) };
            ptr as *mut c_char
        }
        Err(e) => {
            eprintln!("rca_http_get: {}", e);
            std::ptr::null_mut()
        }
    }
}

/// Free a buffer previously returned by an FFI helper.
///
/// # Safety
/// `ptr` must be either null or a pointer returned by an FFI helper in this
/// module, and must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    len += 1;
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    unsafe { std::alloc::dealloc(ptr.cast::<u8>(), layout) };
}

/// HTTPS/HTTP POST. `url_ptr` and `body_ptr` are null-terminated C strings.
/// On success returns a null-terminated C string allocated with the Rust
/// allocator (caller owns; use `rca_free`). On failure prints to stderr and
/// returns NULL.
///
/// # Safety
/// Both pointers must be valid, non-null, null-terminated C strings that
/// stay valid for the duration of the call. The returned pointer must be
/// freed with `rca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_http_post(
    url_ptr: *const c_char,
    body_ptr: *const c_char,
) -> *mut c_char {
    if url_ptr.is_null() || body_ptr.is_null() {
        eprintln!("rca_http_post: null argument");
        return std::ptr::null_mut();
    }
    let url = match unsafe { CStr::from_ptr(url_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("rca_http_post: url is not valid UTF-8");
            return std::ptr::null_mut();
        }
    };
    let body = match unsafe { CStr::from_ptr(body_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("rca_http_post: body is not valid UTF-8");
            return std::ptr::null_mut();
        }
    };
    match crate::runtime::http::http_post(url, body) {
        Ok(resp) => {
            let mut bytes = resp.into_bytes();
            bytes.push(0);
            let len = bytes.len();
            let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
            let ptr = unsafe { std::alloc::alloc(layout) };
            if ptr.is_null() {
                eprintln!("rca_http_post: alloc failed");
                return std::ptr::null_mut();
            }
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len) };
            ptr as *mut c_char
        }
        Err(e) => {
            eprintln!("rca_http_post: {}", e);
            std::ptr::null_mut()
        }
    }
}

// -------- String <-> primitive conversions (Phase 3 native codegen) --------

/// Allocate a null-terminated C string from a Rust string.
/// Caller owns the returned buffer; use `rca_free` to release it.
fn cstring_from(s: String) -> *mut c_char {
    let mut bytes = s.into_bytes();
    bytes.push(0);
    let len = bytes.len();
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len) };
    ptr as *mut c_char
}

/// Format an `Int` as a null-terminated C string (caller owns; use `rca_free`).
///
/// # Safety
///
/// Pure function of `x`; the returned pointer must be freed with `rca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_int_to_str(x: i64) -> *mut c_char {
    cstring_from(x.to_string())
}

/// Format a `Float` (passed as IEEE 754 bits) as a null-terminated C string.
///
/// # Safety
///
/// Pure function of `bits`; the returned pointer must be freed with `rca_free`.
/// Matches RCA `str()` formatting (integer-valued floats get `.0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_float_to_str(bits: i64) -> *mut c_char {
    let x = f64::from_bits(bits as u64);
    cstring_from(crate::runtime::values::Value::Float(x).to_string())
}

/// Format a `Bool` (0 or 1) as a null-terminated C string.
///
/// # Safety
///
/// Pure function of `x`; the returned pointer must be freed with `rca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_bool_to_str(x: i64) -> *mut c_char {
    cstring_from(if x != 0 {
        "true".to_string()
    } else {
        "false".to_string()
    })
}

/// Parse a null-terminated C string as `Int`. Exits the process on parse error.
///
/// # Safety
///
/// `p` must be null or a valid, null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_str_to_int(p: *const c_char) -> i64 {
    if p.is_null() {
        eprintln!("runtime error: int() on null string");
        std::process::exit(1);
    }
    let s = match unsafe { CStr::from_ptr(p) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("runtime error: int() on invalid UTF-8");
            std::process::exit(1);
        }
    };
    match s.trim().parse::<i64>() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("runtime error: int() cannot parse {:?}", s);
            std::process::exit(1);
        }
    }
}

/// Parse a null-terminated C string as `Float` (returns IEEE 754 bits).
///
/// # Safety
///
/// `p` must be null or a valid, null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_str_to_float(p: *const c_char) -> i64 {
    if p.is_null() {
        eprintln!("runtime error: float() on null string");
        std::process::exit(1);
    }
    let s = match unsafe { CStr::from_ptr(p) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("runtime error: float() on invalid UTF-8");
            std::process::exit(1);
        }
    };
    match s.trim().parse::<f64>() {
        Ok(x) => x.to_bits() as i64,
        Err(_) => {
            eprintln!("runtime error: float() cannot parse {:?}", s);
            std::process::exit(1);
        }
    }
}

/// Return current Unix timestamp in milliseconds.
#[unsafe(no_mangle)]
pub extern "C" fn rca_time_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(unix)]
#[repr(C)]
pub struct CTimespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

#[cfg(unix)]
unsafe extern "C" {
    fn nanosleep(req: *const CTimespec, rem: *mut CTimespec) -> i32;
}

/// Sleep for `ms` milliseconds via libc `nanosleep` (avoids std::thread on macOS ARM64).
///
/// # Safety
///
/// This function has no caller-side requirements beyond passing a normal `i64`;
/// it does not dereference any pointers provided by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_time_sleep(ms: i64) -> i64 {
    if ms <= 0 {
        return 0;
    }
    #[cfg(unix)]
    {
        let req = CTimespec {
            tv_sec: ms / 1000,
            tv_nsec: (ms % 1000) * 1_000_000,
        };
        unsafe {
            nanosleep(&req, std::ptr::null_mut());
        }
    }
    #[cfg(windows)]
    {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    0
}

/// Read a file and return its contents as a null-terminated C string.
/// Caller owns the result; free with `rca_free`. Returns NULL on error.
///
/// # Safety
///
/// `path_ptr` must be a valid, non-null, null-terminated C string.
/// The returned pointer must be freed with `rca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_read_file(path_ptr: *const c_char) -> *mut c_char {
    if path_ptr.is_null() {
        eprintln!("rca_read_file: null path");
        return std::ptr::null_mut();
    }
    let path = match unsafe { CStr::from_ptr(path_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("rca_read_file: path is not valid UTF-8");
            return std::ptr::null_mut();
        }
    };
    match std::fs::read_to_string(path) {
        Ok(body) => {
            let mut bytes = body.into_bytes();
            bytes.push(0);
            let len = bytes.len();
            let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
            let ptr = unsafe { std::alloc::alloc(layout) };
            if ptr.is_null() {
                eprintln!("rca_read_file: alloc failed");
                return std::ptr::null_mut();
            }
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len) };
            ptr as *mut c_char
        }
        Err(e) => {
            eprintln!("rca_read_file: {}", e);
            std::ptr::null_mut()
        }
    }
}

/// Write content to a file. Returns 0 on success, 1 on error.
///
/// # Safety
///
/// `path_ptr` and `content_ptr` must be valid, non-null, null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_write_file(
    path_ptr: *const c_char,
    content_ptr: *const c_char,
) -> i64 {
    if path_ptr.is_null() || content_ptr.is_null() {
        eprintln!("rca_write_file: null argument");
        return 1;
    }
    let path = match unsafe { CStr::from_ptr(path_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("rca_write_file: path is not valid UTF-8");
            return 1;
        }
    };
    let content = match unsafe { CStr::from_ptr(content_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            eprintln!("rca_write_file: content is not valid UTF-8");
            return 1;
        }
    };
    match std::fs::write(path, content) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("rca_write_file: {}", e);
            1
        }
    }
}

/// Return 1 if the path exists (file or directory), 0 otherwise.
///
/// # Safety
///
/// `path_ptr` must be a valid, non-null, null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_file_exists(path_ptr: *const c_char) -> i64 {
    if path_ptr.is_null() {
        return 0;
    }
    let path = match unsafe { CStr::from_ptr(path_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    if std::path::Path::new(path).exists() {
        1
    } else {
        0
    }
}

fn alloc_c_string(s: &str) -> *mut c_char {
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0);
    let len = bytes.len();
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    unsafe {
        let ptr = std::alloc::alloc(layout);
        if ptr.is_null() {
            return std::ptr::null_mut();
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len);
        ptr as *mut c_char
    }
}

/// Trim leading/trailing whitespace from a string.
///
/// # Safety
///
/// `s_ptr` must be a valid, non-null, null-terminated C string.
/// The returned pointer must be freed with `rca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_str_trim(s_ptr: *const c_char) -> *mut c_char {
    if s_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(s_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    alloc_c_string(s.trim())
}

/// Replace all occurrences of `target` with `rep` in `s`.
///
/// # Safety
///
/// `s_ptr`, `target_ptr`, and `rep_ptr` must be valid, non-null,
/// null-terminated C strings. The returned pointer must be freed with `rca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_str_replace(
    s_ptr: *const c_char,
    target_ptr: *const c_char,
    rep_ptr: *const c_char,
) -> *mut c_char {
    if s_ptr.is_null() || target_ptr.is_null() || rep_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(s_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let target = match unsafe { CStr::from_ptr(target_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let rep = match unsafe { CStr::from_ptr(rep_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    alloc_c_string(&s.replace(target, rep))
}

/// Split `s` by `sep` and return a native list of strings.
///
/// Layout: `[refcount: i64, length: i64, elem0: *const c_char, ...]`.
///
/// # Safety
///
/// `s_ptr` and `sep_ptr` must be valid, non-null, null-terminated C strings.
/// The returned pointer must be freed by the caller using the same layout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_str_split(s_ptr: *const c_char, sep_ptr: *const c_char) -> *mut i64 {
    if s_ptr.is_null() || sep_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let s = match unsafe { CStr::from_ptr(s_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let sep = match unsafe { CStr::from_ptr(sep_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let parts: Vec<&str> = s.split(sep).collect();
    let n = parts.len();
    let size = (n + 2) * std::mem::size_of::<i64>();
    let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
    let base = unsafe { std::alloc::alloc(layout) as *mut i64 };
    if base.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *base = 1;
        *base.add(1) = n as i64;
        for (i, part) in parts.iter().enumerate() {
            let sp = alloc_c_string(part);
            *base.add(i + 2) = sp as i64;
        }
    }
    base
}

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};

static TCP_SOCKETS: Mutex<Option<HashMap<i64, std::net::TcpStream>>> = Mutex::new(None);
static UDP_SOCKETS: Mutex<Option<HashMap<i64, std::net::UdpSocket>>> = Mutex::new(None);
static NEXT_FD: AtomicI64 = AtomicI64::new(1);

fn next_fd() -> i64 {
    NEXT_FD.fetch_add(1, Ordering::SeqCst)
}

fn tcp_registry() -> std::sync::MutexGuard<'static, Option<HashMap<i64, std::net::TcpStream>>> {
    TCP_SOCKETS.lock().unwrap_or_else(|e| e.into_inner())
}

fn udp_registry() -> std::sync::MutexGuard<'static, Option<HashMap<i64, std::net::UdpSocket>>> {
    UDP_SOCKETS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Connect to `host:port` over TCP. Returns a positive fd handle on success, 0 on error.
///
/// # Safety
///
/// `host_ptr` must be a valid, non-null, null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_tcp_connect(host_ptr: *const c_char, port: i64) -> i64 {
    if host_ptr.is_null() {
        eprintln!("rca_tcp_connect: null host");
        return 0;
    }
    let host = match unsafe { CStr::from_ptr(host_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let addr = format!("{}:{}", host, port);
    match std::net::TcpStream::connect(&addr) {
        Ok(stream) => {
            let fd = next_fd();
            let mut reg = tcp_registry();
            reg.get_or_insert_with(HashMap::new).insert(fd, stream);
            fd
        }
        Err(e) => {
            eprintln!("rca_tcp_connect: {}", e);
            0
        }
    }
}

/// Send `data_ptr` bytes over TCP socket `fd`. Returns bytes sent, or -1 on error.
///
/// # Safety
///
/// `data_ptr` must be a valid, non-null, null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_tcp_send(fd: i64, data_ptr: *const c_char) -> i64 {
    if data_ptr.is_null() {
        return -1;
    }
    let data = match unsafe { CStr::from_ptr(data_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let mut reg = tcp_registry();
    let stream = match reg.as_mut().and_then(|m| m.get_mut(&fd)) {
        Some(s) => s,
        None => return -1,
    };
    use std::io::Write;
    match stream.write(data.as_bytes()) {
        Ok(n) => n as i64,
        Err(_) => -1,
    }
}

/// Receive up to `max_bytes` from TCP socket `fd`. Returns a null-terminated
/// C string; caller owns it (free with `rca_free`). Returns NULL on error.
///
/// # Safety
///
/// Returned pointer must be freed with `rca_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_tcp_recv(fd: i64, max_bytes: i64) -> *mut c_char {
    if max_bytes <= 0 {
        return std::ptr::null_mut();
    }
    let mut reg = tcp_registry();
    let stream = match reg.as_mut().and_then(|m| m.get_mut(&fd)) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    use std::io::Read;
    let mut buf = vec![0u8; max_bytes as usize];
    match stream.read(&mut buf) {
        Ok(n) => {
            let s = String::from_utf8_lossy(&buf[..n]).to_string();
            alloc_c_string(&s)
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Close TCP socket `fd`. Returns 0 (always succeeds).
#[unsafe(no_mangle)]
pub extern "C" fn rca_tcp_close(fd: i64) -> i64 {
    let mut reg = tcp_registry();
    if let Some(m) = reg.as_mut() {
        m.remove(&fd);
    }
    0
}

unsafe extern "C" {
    fn malloc(size: usize) -> *mut std::ffi::c_void;
}

/// # Safety
/// `host_ptr` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_udp_bind(host_ptr: *const c_char, port: i64) -> i64 {
    if host_ptr.is_null() {
        return 0;
    }
    let host = match unsafe { CStr::from_ptr(host_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    match std::net::UdpSocket::bind(format!("{}:{}", host, port)) {
        Ok(sock) => {
            let fd = next_fd();
            udp_registry()
                .get_or_insert_with(HashMap::new)
                .insert(fd, sock);
            fd
        }
        Err(e) => {
            eprintln!("rca_udp_bind: {}", e);
            0
        }
    }
}

/// # Safety
/// `host_ptr` and `data_ptr` must be valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rca_udp_send(
    fd: i64,
    host_ptr: *const c_char,
    port: i64,
    data_ptr: *const c_char,
) -> i64 {
    if host_ptr.is_null() || data_ptr.is_null() {
        return -1;
    }
    let host = match unsafe { CStr::from_ptr(host_ptr) }.to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let data = unsafe { CStr::from_ptr(data_ptr) }.to_bytes();
    let sock = {
        let reg = udp_registry();
        match reg.as_ref().and_then(|m| m.get(&fd)).map(|s| s.try_clone()) {
            Some(Ok(s)) => s,
            _ => return -1,
        }
    };
    match sock.send_to(data, format!("{}:{}", host, port)) {
        Ok(n) => n as i64,
        Err(_) => -1,
    }
}

/// Returns tuple block `[1][3][data][src_host][src_port]` or NULL on error.
#[unsafe(no_mangle)]
pub extern "C" fn rca_udp_recv(fd: i64, max_bytes: i64) -> *mut i64 {
    if max_bytes <= 0 {
        return std::ptr::null_mut();
    }
    let sock = {
        let reg = udp_registry();
        match reg.as_ref().and_then(|m| m.get(&fd)).map(|s| s.try_clone()) {
            Some(Ok(s)) => s,
            _ => return std::ptr::null_mut(),
        }
    };
    let mut buf = vec![0u8; max_bytes as usize];
    let (n, src) = match sock.recv_from(&mut buf) {
        Ok(v) => v,
        Err(_) => return std::ptr::null_mut(),
    };
    let data = alloc_c_string(&String::from_utf8_lossy(&buf[..n]));
    let host = alloc_c_string(&src.ip().to_string());
    let block = unsafe { malloc(5 * 8) } as *mut i64;
    if block.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *block.add(0) = 1;
        *block.add(1) = 3;
        *block.add(2) = data as i64;
        *block.add(3) = host as i64;
        *block.add(4) = src.port() as i64;
    }
    block
}

#[unsafe(no_mangle)]
pub extern "C" fn rca_udp_close(fd: i64) -> i64 {
    if let Some(m) = udp_registry().as_mut() {
        m.remove(&fd);
    }
    0
}
