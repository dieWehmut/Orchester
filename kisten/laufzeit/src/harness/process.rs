use std::{
    borrow::Cow,
    fmt,
    io::{self, Read},
};

const READ_BUFFER_BYTES: usize = 8 * 1024;

/// Bounded bytes captured from a process stream.
///
/// The retained bytes are intentionally not included in `Debug` output. The
/// total count describes the complete input, even when only a prefix is kept.
#[derive(Clone, PartialEq, Eq)]
pub struct BoundedOutput {
    bytes: Vec<u8>,
    total_bytes: u64,
    truncated: bool,
}

impl BoundedOutput {
    pub(crate) fn from_parts(bytes: Vec<u8>, total_bytes: u64) -> Self {
        Self {
            truncated: total_bytes > bytes.len() as u64,
            bytes,
            total_bytes,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn text_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }
}

impl fmt::Debug for BoundedOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundedOutput")
            .field("retained_bytes", &self.bytes.len())
            .field("total_bytes", &self.total_bytes)
            .field("truncated", &self.truncated)
            .finish()
    }
}

/// Read a stream incrementally while retaining at most `max_bytes`.
pub fn capture_bounded<R: Read>(mut reader: R, max_bytes: usize) -> io::Result<BoundedOutput> {
    let mut bytes = Vec::with_capacity(max_bytes.min(READ_BUFFER_BYTES));
    let mut buffer = [0_u8; READ_BUFFER_BYTES];
    let mut total_bytes = 0_u64;

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        total_bytes = total_bytes
            .checked_add(u64::try_from(read).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "stream byte count overflow")
            })?)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "stream byte count overflow")
            })?;

        let remaining = max_bytes.saturating_sub(bytes.len());
        if remaining > 0 {
            bytes.extend_from_slice(&buffer[..read.min(remaining)]);
        }
    }

    Ok(BoundedOutput {
        truncated: total_bytes > bytes.len() as u64,
        bytes,
        total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read};

    use super::capture_bounded;

    struct ChunkedReader {
        chunks: Vec<Vec<u8>>,
        next: usize,
    }

    impl ChunkedReader {
        fn new(chunks: &[&[u8]]) -> Self {
            Self {
                chunks: chunks.iter().map(|chunk| chunk.to_vec()).collect(),
                next: 0,
            }
        }
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let Some(chunk) = self.chunks.get(self.next) else {
                return Ok(0);
            };
            assert!(chunk.len() <= buffer.len());
            buffer[..chunk.len()].copy_from_slice(chunk);
            self.next += 1;
            Ok(chunk.len())
        }
    }

    #[test]
    fn captures_multiple_reads_and_tracks_the_total() {
        let output = capture_bounded(ChunkedReader::new(&[b"alpha ", b"beta", b" gamma"]), 64)
            .expect("capture output");

        assert_eq!(output.bytes(), b"alpha beta gamma");
        assert_eq!(output.total_bytes(), 16);
        assert!(!output.truncated());
    }

    #[test]
    fn retains_only_the_limit_while_counting_the_complete_stream() {
        let output = capture_bounded(ChunkedReader::new(&[b"1234", b"5678", b"90"]), 6)
            .expect("capture output");

        assert_eq!(output.bytes(), b"123456");
        assert_eq!(output.total_bytes(), 10);
        assert!(output.truncated());
    }

    #[test]
    fn captures_an_empty_stream_without_truncation() {
        let output = capture_bounded(io::empty(), 0).expect("capture output");

        assert!(output.bytes().is_empty());
        assert_eq!(output.total_bytes(), 0);
        assert!(!output.truncated());
    }

    #[test]
    fn converts_invalid_utf8_lossily_without_debug_exposing_content() {
        let output = capture_bounded(&b"secret:\xff"[..], 32).expect("capture output");

        assert_eq!(output.text_lossy(), "secret:\u{fffd}");
        let debug = format!("{output:?}");
        assert!(debug.contains("total_bytes: 8"));
        assert!(!debug.contains("secret"));
    }
}
