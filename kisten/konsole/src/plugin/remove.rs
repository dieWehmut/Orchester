use std::fs;
use std::path::Path;

use orchester_verzeichnis::{PluginInfo, load_agent_plugin};
use thiserror::Error;

use super::install::{receipt, store, valid_name};

pub enum RemoveOutcome {
    Removed(PluginInfo),
    NotInstalled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RemoveError {
    #[error("plugin name is invalid")]
    InvalidName,
    #[error("managed plugin store is unavailable")]
    StoreUnavailable,
    #[error("managed plugin ownership could not be verified")]
    OwnershipUnverified,
    #[error("managed plugin removal failed")]
    RemovalFailed,
}

pub fn remove(orchester_home: &Path, name: &str) -> Result<RemoveOutcome, RemoveError> {
    if !valid_name(name) {
        return Err(RemoveError::InvalidName);
    }
    let paths =
        store::managed_paths(orchester_home, name).map_err(|_| RemoveError::StoreUnavailable)?;
    let scope_exists =
        store::verify_directory_tree(&paths.scope).map_err(|_| RemoveError::StoreUnavailable)?;
    let receipt_root_exists = store::verify_directory_tree(&paths.receipt_root)
        .map_err(|_| RemoveError::StoreUnavailable)?;
    let target_exists = scope_exists && path_exists(&paths.target)?;
    let receipt_exists = receipt_root_exists && path_exists(&paths.receipt)?;

    match (target_exists, receipt_exists) {
        (false, false) => return Ok(RemoveOutcome::NotInstalled),
        (true, false) => return Err(RemoveError::OwnershipUnverified),
        (false, true) => {
            receipt::validate_stale(&paths.receipt, name)
                .map_err(|_| RemoveError::OwnershipUnverified)?;
            fs::remove_file(&paths.receipt).map_err(|_| RemoveError::RemovalFailed)?;
            return Ok(RemoveOutcome::NotInstalled);
        }
        (true, true) => {}
    }

    let loaded = load_agent_plugin(&paths.target).map_err(|_| RemoveError::OwnershipUnverified)?;
    receipt::verify(&paths.receipt, &paths.target, loaded.info())
        .map_err(|_| RemoveError::OwnershipUnverified)?;
    let info = loaded.info().clone();

    store::ensure_directory_tree(&paths.trash_root).map_err(|_| RemoveError::StoreUnavailable)?;
    let trash = store::create_unique_directory(&paths.trash_root, "remove")
        .map_err(|_| RemoveError::StoreUnavailable)?;
    let isolated = trash.join("package");
    fs::rename(&paths.target, &isolated).map_err(|_| RemoveError::RemovalFailed)?;

    let isolated_valid = load_agent_plugin(&isolated)
        .ok()
        .and_then(|loaded| receipt::verify(&paths.receipt, &isolated, loaded.info()).ok())
        .is_some();
    if !isolated_valid {
        let _ = fs::rename(&isolated, &paths.target);
        let _ = fs::remove_dir(&trash);
        return Err(RemoveError::OwnershipUnverified);
    }
    if fs::remove_dir_all(&isolated).is_err() {
        return Err(RemoveError::RemovalFailed);
    }
    let _ = fs::remove_dir(&trash);
    fs::remove_file(&paths.receipt).map_err(|_| RemoveError::RemovalFailed)?;
    Ok(RemoveOutcome::Removed(info))
}

fn path_exists(path: &Path) -> Result<bool, RemoveError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(RemoveError::StoreUnavailable),
    }
}
