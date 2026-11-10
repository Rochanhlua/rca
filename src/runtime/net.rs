//! TCP socket wrapper for pure Rust networking.
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct Socket {
    pub stream: Arc<Mutex<TcpStream>>,
}

impl Clone for Socket {
    fn clone(&self) -> Self {
        Self {
            stream: Arc::clone(&self.stream),
        }
    }
}

impl PartialEq for Socket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.stream, &other.stream)
    }
}
