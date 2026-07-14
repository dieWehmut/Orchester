use std::io::Read;
use std::path::Path;

use zeroize::Zeroizing;

use super::ConfigError;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix::open_validated_file;
#[cfg(windows)]
use windows::open_validated_file;

pub(super) const MAX_SOURCE_BYTES: usize = 1024 * 1024;

pub(super) fn read_protected_file(path: &Path) -> Result<Zeroizing<String>, ConfigError> {
    let mut file = open_validated_file(path)?;
    read_bounded_source(&mut file)
}

fn read_bounded_source<R: Read>(reader: &mut R) -> Result<Zeroizing<String>, ConfigError> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(MAX_SOURCE_BYTES + 1));
    reader
        .take((MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ConfigError::ProtectedFileIo)?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(ConfigError::ProtectedFileTooLarge);
    }
    let owned_bytes = std::mem::take(&mut *bytes);
    match String::from_utf8(owned_bytes) {
        Ok(source) => Ok(Zeroizing::new(source)),
        Err(error) => {
            let _invalid_source = Zeroizing::new(error.into_bytes());
            Err(ConfigError::ProtectedFileInvalidUtf8)
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn open_validated_file(_path: &Path) -> Result<std::fs::File, ConfigError> {
    Err(ConfigError::ProtectedFileSecurity)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn bounded_reader_accepts_exactly_one_mibibyte() {
        let bytes = vec![b' '; MAX_SOURCE_BYTES];

        let source = read_bounded_source(&mut Cursor::new(bytes)).unwrap();

        assert_eq!(source.len(), MAX_SOURCE_BYTES);
    }

    #[test]
    fn bounded_reader_rejects_more_than_one_mibibyte() {
        let bytes = vec![b' '; MAX_SOURCE_BYTES + 1];

        let error = read_bounded_source(&mut Cursor::new(bytes)).unwrap_err();

        assert!(matches!(error, ConfigError::ProtectedFileTooLarge));
    }

    #[test]
    fn bounded_reader_rejects_invalid_utf8_without_echoing_source() {
        let bytes = b"do-not-echo\xff";

        let error = read_bounded_source(&mut Cursor::new(bytes)).unwrap_err();

        assert!(matches!(error, ConfigError::ProtectedFileInvalidUtf8));
        assert!(!error.to_string().contains("do-not-echo"));
    }
}
