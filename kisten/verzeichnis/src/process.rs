//! Shared process resolution lives below the registry so adapters use the same
//! shell-free Windows npm shim handling as local CLI launches.
pub use orchester_vertrag::process::*;
