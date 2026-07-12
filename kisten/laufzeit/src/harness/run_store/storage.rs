use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

use super::StoreError;

pub(super) fn enable_wal_mode(connection: &Connection) -> Result<(), StoreError> {
    let mut last_busy = None;
    for _ in 0..100 {
        match connection.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        }) {
            Ok(mode) if mode.eq_ignore_ascii_case("wal") => return Ok(()),
            Ok(_) => {
                return Err(StoreError::Invariant(
                    "state database could not enter WAL mode".into(),
                ))
            }
            Err(error) if sqlite_is_busy(&error) => {
                last_busy = Some(error);
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(StoreError::Database(error)),
        }
    }
    Err(StoreError::Database(
        last_busy.unwrap_or_else(|| rusqlite::Error::InvalidQuery),
    ))
}

fn sqlite_is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(details, _)
            if matches!(
                details.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

#[cfg(unix)]
pub(super) fn ensure_private_state_dir(path: &Path, created: bool) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    if created {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(StoreError::Io)?;
    }
    let mode = std::fs::metadata(path)
        .map_err(StoreError::Io)?
        .permissions()
        .mode()
        & 0o777;
    if mode == 0o700 {
        Ok(())
    } else {
        Err(StoreError::InsecurePermissions)
    }
}

#[cfg(windows)]
pub(super) fn ensure_private_state_dir(path: &Path, _created: bool) -> Result<(), StoreError> {
    if crate::harness::config::check_permissions(path)
        .into_iter()
        .all(|finding| finding.secure)
    {
        Ok(())
    } else {
        Err(StoreError::InsecurePermissions)
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn ensure_private_state_dir(_path: &Path, _created: bool) -> Result<(), StoreError> {
    Err(StoreError::InsecurePermissions)
}

#[cfg(unix)]
pub(super) fn ensure_private_state_file(path: &Path, created: bool) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    if created {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(StoreError::Io)?;
    }
    let mode = std::fs::metadata(path)
        .map_err(StoreError::Io)?
        .permissions()
        .mode()
        & 0o777;
    if mode == 0o600 {
        Ok(())
    } else {
        Err(StoreError::InsecurePermissions)
    }
}

#[cfg(unix)]
pub(super) fn ensure_private_state_sidecars(
    path: &Path,
    wal_existed: bool,
    shm_existed: bool,
) -> Result<(), StoreError> {
    for (suffix, existed) in [("-wal", wal_existed), ("-shm", shm_existed)] {
        let sidecar = state_sidecar(path, suffix);
        if sidecar.exists() {
            ensure_private_state_file(&sidecar, !existed)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn ensure_private_state_sidecars(
    _path: &Path,
    _wal_existed: bool,
    _shm_existed: bool,
) -> Result<(), StoreError> {
    Ok(())
}

pub(super) fn state_sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    value.into()
}

#[cfg(windows)]
pub(super) fn ensure_private_state_file(path: &Path, _created: bool) -> Result<(), StoreError> {
    if crate::harness::config::check_permissions(path)
        .into_iter()
        .all(|finding| finding.secure)
    {
        Ok(())
    } else {
        Err(StoreError::InsecurePermissions)
    }
}

#[cfg(not(any(unix, windows)))]
pub(super) fn ensure_private_state_file(_path: &Path, _created: bool) -> Result<(), StoreError> {
    Err(StoreError::InsecurePermissions)
}
