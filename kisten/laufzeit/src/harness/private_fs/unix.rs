use std::fs::File;
use std::mem::MaybeUninit;
use std::os::unix::io::AsRawFd;

use super::PrivateHandleError;

pub(crate) fn validate_private_handle(
    file: &File,
    expect_directory: bool,
) -> Result<(), PrivateHandleError> {
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(PrivateHandleError::Io);
    }
    let stat = unsafe { stat.assume_init() };
    let (expected_type, expected_mode) = if expect_directory {
        (libc::S_IFDIR, 0o700)
    } else {
        (libc::S_IFREG, 0o600)
    };
    if stat.st_mode & libc::S_IFMT != expected_type
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o7777 != expected_mode
    {
        return Err(PrivateHandleError::Security);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchester-private-handle-unix-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn exact_owned_file_and_directory_modes_are_required() {
        let root = TempDir::new();
        let directory = File::open(&root.0).unwrap();
        let path = root.0.join("object");
        fs::write(&path, b"body").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::open(&path).unwrap();

        assert_eq!(validate_private_handle(&directory, true), Ok(()));
        assert_eq!(validate_private_handle(&file, false), Ok(()));
        assert_eq!(
            validate_private_handle(&file, true),
            Err(PrivateHandleError::Security)
        );

        fs::set_permissions(&root.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            validate_private_handle(&directory, true),
            Err(PrivateHandleError::Security)
        );

        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert_eq!(
            validate_private_handle(&file, false),
            Err(PrivateHandleError::Security)
        );
    }
}
