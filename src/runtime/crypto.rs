//! Cryptographic hashing implemented in pure Rust using the audited `sha2` crate.
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256, Sha512};

type HmacSha256 = Hmac<Sha256>;

pub fn sha256(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in result {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

pub fn sha512(s: &str) -> String {
    let mut hasher = Sha512::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    let mut hex = String::with_capacity(128);
    for byte in result {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

pub fn hmac_sha256(key: &str, msg: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC accepts any key length");
    mac.update(msg.as_bytes());
    let result = mac.finalize().into_bytes();
    let mut hex = String::with_capacity(64);
    for byte in result {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}
