//! Receipt-owned plugin mutations shared by CLI and desktop.
mod install;
mod remove;

pub use install::{install, InstallError};
pub use remove::{remove, RemoveError, RemoveOutcome};
