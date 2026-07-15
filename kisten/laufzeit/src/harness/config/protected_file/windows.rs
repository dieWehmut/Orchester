use std::fs::{File, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_SHARE_READ,
};

use super::ConfigError;
use crate::harness::private_fs::{PrivateHandleError, validate_private_handle};

pub(super) fn open_validated_file(path: &Path) -> Result<File, ConfigError> {
    let file = OpenOptions::new()
        .read(true)
        .access_mode(FILE_GENERIC_READ)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| ConfigError::ProtectedFileIo)?;
    validate_private_handle(&file, false).map_err(map_validation_error)?;
    Ok(file)
}

fn map_validation_error(error: PrivateHandleError) -> ConfigError {
    match error {
        PrivateHandleError::Io | PrivateHandleError::Security => {
            ConfigError::ProtectedFileSecurity
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;

    use super::*;
    use crate::harness::config::protected_file::{read_bounded_source, read_protected_file};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchester-protected-file-windows-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn join(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn system_tool(relative: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join(relative)
    }

    fn current_sid() -> &'static str {
        static CURRENT_SID: OnceLock<String> = OnceLock::new();
        CURRENT_SID.get_or_init(|| {
            let output = Command::new(system_tool("WindowsPowerShell\\v1.0\\powershell.exe"))
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value",
                ])
                .output()
                .unwrap();
            assert!(output.status.success());
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        })
    }

    fn apply_acl(path: &Path, grants: &[String]) {
        let output = Command::new(system_tool("icacls.exe"))
            .arg(path)
            .args(["/inheritance:r", "/grant:r"])
            .args(grants)
            .output()
            .unwrap();
        assert!(output.status.success());
    }

    fn write_strict_file(root: &TempDir, source: &[u8]) -> std::path::PathBuf {
        let path = root.join("config.jsonc");
        fs::write(&path, source).unwrap();
        apply_acl(
            &path,
            &[
                format!("*{}:(F)", current_sid()),
                "*S-1-5-18:(F)".to_owned(),
                "*S-1-5-32-544:(F)".to_owned(),
            ],
        );
        path
    }

    fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        match std::os::windows::fs::symlink_dir(target, link) {
            Ok(()) => Ok(()),
            Err(symlink_error) => {
                let output = Command::new(system_tool("cmd.exe"))
                    .args(["/C", "mklink", "/J"])
                    .arg(link)
                    .arg(target)
                    .output()?;
                if output.status.success() {
                    Ok(())
                } else {
                    Err(symlink_error)
                }
            }
        }
    }

    #[test]
    fn strict_acl_file_is_accepted_by_real_handle_validation() {
        let root = TempDir::new();
        let path = write_strict_file(&root, b"{}");

        let source = read_protected_file(&path).unwrap();

        assert_eq!(&*source, "{}");
    }

    #[test]
    fn builtin_users_allow_is_rejected_by_real_handle_validation() {
        let root = TempDir::new();
        let path = write_strict_file(&root, b"{}");
        let output = Command::new(system_tool("icacls.exe"))
            .arg(&path)
            .args(["/grant", "*S-1-5-32-545:(R)"])
            .output()
            .unwrap();
        assert!(output.status.success());

        assert!(matches!(
            read_protected_file(&path),
            Err(ConfigError::ProtectedFileSecurity)
        ));
    }

    #[test]
    fn directory_and_junction_are_rejected_by_real_handle_validation() {
        let root = TempDir::new();
        let target = root.join("target");
        let junction = root.join("junction");
        fs::create_dir(&target).unwrap();
        create_directory_link(&target, &junction).unwrap();

        assert!(read_protected_file(&target).is_err());
        assert!(read_protected_file(&junction).is_err());

        fs::remove_dir(&junction).unwrap();
    }

    #[test]
    fn validated_handle_blocks_write_and_replacement_opens() {
        let root = TempDir::new();
        let path = write_strict_file(&root, b"original");
        let renamed = root.join("renamed.jsonc");
        let mut file = open_validated_file(&path).unwrap();

        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::rename(&path, &renamed).is_err());
        let source = read_bounded_source(&mut file).unwrap();

        assert_eq!(&*source, "original");
    }
}
