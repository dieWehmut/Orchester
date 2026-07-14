use std::fs::{File, OpenOptions};
use std::mem::MaybeUninit;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;

use super::ConfigError;

const OPEN_FLAGS: i32 = libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK;

pub(super) fn open_validated_file(path: &Path) -> Result<File, ConfigError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(OPEN_FLAGS)
        .open(path)
        .map_err(|_| ConfigError::ProtectedFileIo)?;

    let mut stat = MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(ConfigError::ProtectedFileIo);
    }
    let stat = unsafe { stat.assume_init() };
    if (stat.st_mode & libc::S_IFMT) != libc::S_IFREG
        || stat.st_uid != unsafe { libc::geteuid() }
        || (stat.st_mode & 0o7777) != 0o600
    {
        return Err(ConfigError::ProtectedFileSecurity);
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::harness::config::protected_file::{read_bounded_source, read_protected_file};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchester-protected-file-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
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

    fn write_secure(path: &std::path::Path, source: &[u8]) {
        fs::write(path, source).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn validated_open_accepts_owned_regular_file_with_mode_0600() {
        let root = TempDir::new();
        let path = root.join("config.jsonc");
        write_secure(&path, b"{}");

        let source = read_protected_file(&path).unwrap();

        assert_eq!(&*source, "{}");
    }

    #[test]
    fn validated_open_rejects_group_or_world_permissions() {
        let root = TempDir::new();
        let path = root.join("config.jsonc");
        write_secure(&path, b"{}");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        assert!(matches!(
            read_protected_file(&path),
            Err(ConfigError::ProtectedFileSecurity)
        ));
    }

    #[test]
    fn validated_open_rejects_special_permission_bits() {
        let root = TempDir::new();
        let path = root.join("config.jsonc");
        write_secure(&path, b"{}");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o1600)).unwrap();

        assert!(matches!(
            read_protected_file(&path),
            Err(ConfigError::ProtectedFileSecurity)
        ));
    }

    #[test]
    fn fifo_is_rejected_without_blocking() {
        assert_ne!(OPEN_FLAGS & libc::O_NONBLOCK, 0);
        let root = TempDir::new();
        let path = root.join("config.fifo");
        let raw_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(raw_path.as_ptr(), 0o600) }, 0);

        assert!(matches!(
            read_protected_file(&path),
            Err(ConfigError::ProtectedFileSecurity)
        ));
    }

    #[test]
    fn validated_open_rejects_directory_and_final_symlink() {
        let root = TempDir::new();
        let target = root.join("target.jsonc");
        let link = root.join("link.jsonc");
        write_secure(&target, b"{}");
        symlink(&target, &link).unwrap();

        assert!(read_protected_file(&root.0).is_err());
        assert!(read_protected_file(&link).is_err());
    }

    #[test]
    fn read_uses_open_file_after_path_is_replaced() {
        let root = TempDir::new();
        let path = root.join("config.jsonc");
        let original = root.join("original.jsonc");
        write_secure(&path, b"original");
        let mut file = open_validated_file(&path).unwrap();

        fs::rename(&path, &original).unwrap();
        write_secure(&path, b"replacement");
        let source = read_bounded_source(&mut file).unwrap();

        assert_eq!(&*source, "original");
    }
}
