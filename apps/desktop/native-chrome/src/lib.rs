#![deny(unsafe_code)]

#[cfg(any(windows, test))]
mod caption_geometry;
// This is the sole FFI boundary. The Tauri shell keeps forbid(unsafe_code).
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_caption;

#[cfg(windows)]
pub use windows_caption::install;
