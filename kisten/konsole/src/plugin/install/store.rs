use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::InstallError;

pub struct InstallTransaction {
    staging: PathBuf,
    target: PathBuf,
    receipt: PathBuf,
}

impl InstallTransaction {
    pub fn new(orchester_home: &Path, name: &str) -> Result<Self, InstallError> {
        if !orchester_home.is_absolute() {
            return Err(InstallError::StagingUnavailable);
        }
        let npm_root = orchester_home.join("plugins").join("npm");
        let staging_root = npm_root.join(".staging");
        let receipt_root = npm_root.join(".receipts");
        let scope = npm_root.join("node_modules").join("@orchester");
        ensure_directory_tree(&staging_root)?;
        ensure_directory_tree(&receipt_root)?;
        ensure_directory_tree(&scope)?;
        let target = scope.join(name);
        let receipt = receipt_root.join(format!("{name}.json"));
        if fs::symlink_metadata(&target).is_ok() || fs::symlink_metadata(&receipt).is_ok() {
            return Err(InstallError::AlreadyInstalled);
        }
        let staging = create_staging_directory(&staging_root)?;
        Ok(Self {
            staging,
            target,
            receipt,
        })
    }

    pub fn staging_path(&self) -> &Path {
        &self.staging
    }

    pub fn activate(&mut self, package: &Path, receipt: &Path) -> Result<(), InstallError> {
        if fs::symlink_metadata(&self.target).is_ok() || fs::symlink_metadata(&self.receipt).is_ok()
        {
            return Err(InstallError::AlreadyInstalled);
        }
        fs::rename(package, &self.target).map_err(|_| InstallError::ActivationFailed)?;
        if fs::rename(receipt, &self.receipt).is_err() {
            let _ = fs::rename(&self.target, package);
            return Err(InstallError::ActivationFailed);
        }
        Ok(())
    }
}

impl Drop for InstallTransaction {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.staging);
    }
}

fn ensure_directory_tree(path: &Path) -> Result<(), InstallError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                current.push(component.as_os_str());
                continue;
            }
            Component::Normal(_) => current.push(component.as_os_str()),
            Component::CurDir | Component::ParentDir => {
                return Err(InstallError::StagingUnavailable);
            }
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
            Ok(_) => return Err(InstallError::StagingUnavailable),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|_| InstallError::StagingUnavailable)?;
                let metadata =
                    fs::symlink_metadata(&current).map_err(|_| InstallError::StagingUnavailable)?;
                if !metadata.is_dir() || is_link_or_reparse(&metadata) {
                    return Err(InstallError::StagingUnavailable);
                }
            }
            Err(_) => return Err(InstallError::StagingUnavailable),
        }
    }
    Ok(())
}

fn create_staging_directory(root: &Path) -> Result<PathBuf, InstallError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| InstallError::StagingUnavailable)?
        .as_nanos();
    for attempt in 0..16_u8 {
        let path = root.join(format!(
            "install-{}-{timestamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(InstallError::StagingUnavailable),
        }
    }
    Err(InstallError::StagingUnavailable)
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
