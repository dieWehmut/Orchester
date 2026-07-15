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
pub(crate) use unix::validate_private_handle;
#[cfg(windows)]
pub(crate) use windows::validate_private_handle;

#[cfg(not(any(unix, windows)))]
pub(crate) fn validate_private_handle(
    _file: &std::fs::File,
    _expect_directory: bool,
) -> Result<(), PrivateHandleError> {
    Err(PrivateHandleError::Security)
}
