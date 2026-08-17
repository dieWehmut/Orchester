use std::io::{self, Write};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundedJsonError {
    LimitExceeded,
    Serialization,
}

pub(super) fn to_bounded_vec<T: Serialize>(
    value: &T,
    limit: usize,
) -> Result<Vec<u8>, BoundedJsonError> {
    let mut output = BoundedBuffer::new(limit);
    serde_json::to_writer(&mut output, value).map_err(|_| {
        if output.exceeded {
            BoundedJsonError::LimitExceeded
        } else {
            BoundedJsonError::Serialization
        }
    })?;
    Ok(output.bytes)
}

pub(super) fn fits<T: Serialize>(value: &T, limit: usize) -> bool {
    let mut counter = BoundedCounter::new(limit);
    serde_json::to_writer(&mut counter, value).is_ok()
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl BoundedBuffer {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(8 * 1024)),
            limit,
            exceeded: false,
        }
    }
}

impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return Err(io::Error::other("bounded JSON buffer exceeded"));
        };
        if next_len > self.limit {
            self.exceeded = true;
            return Err(io::Error::other("bounded JSON buffer exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct BoundedCounter {
    bytes: usize,
    limit: usize,
}

impl BoundedCounter {
    const fn new(limit: usize) -> Self {
        Self { bytes: 0, limit }
    }
}

impl Write for BoundedCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.checked_add(bytes.len()) else {
            return Err(io::Error::other("bounded JSON counter exceeded"));
        };
        if next_len > self.limit {
            return Err(io::Error::other("bounded JSON counter exceeded"));
        }
        self.bytes = next_len;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
