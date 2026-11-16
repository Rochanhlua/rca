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
