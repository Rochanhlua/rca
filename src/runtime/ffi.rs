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
