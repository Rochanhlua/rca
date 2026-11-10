//! Cryptographic hashing implemented in pure Rust using the audited `sha2` crate.
use sha2::{Digest, Sha256};

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
