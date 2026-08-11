#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrivateHandleError {
    Io,
    Security,
}

#[cfg(unix)]
pub(crate) use unix::{create_private_dir_all, validate_private_handle, write_private_file};
#[cfg(windows)]
pub(crate) use windows::{create_private_dir_all, validate_private_handle, write_private_file};

#[cfg(not(any(unix, windows)))]
pub(crate) fn validate_private_handle(
    _file: &std::fs::File,
    _expect_directory: bool,
) -> Result<(), PrivateHandleError> {
    Err(PrivateHandleError::Security)
}

/// Refusing beats writing a secret somewhere privacy cannot be established.
#[cfg(not(any(unix, windows)))]
pub(crate) fn create_private_dir_all(_path: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "creating a user-only directory is unsupported on this platform",
    ))
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn write_private_file(_path: &std::path::Path, _contents: &str) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "creating a user-only file is unsupported on this platform",
    ))
}
