use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    ACE_HEADER, ACL, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION, GetAce, GetAclInformation,
    INHERIT_ONLY_ACE, IsValidAcl, IsValidSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    PSID,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_TYPE_DISK, GetFileInformationByHandle, GetFileType,
};

use super::PrivateHandleError;

pub(crate) fn validate_private_handle(
    file: &File,
    expect_directory: bool,
) -> Result<(), PrivateHandleError> {
    let handle = file.as_raw_handle();
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(handle, &mut information) } == 0 {
        return Err(PrivateHandleError::Io);
    }
    if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != expect_directory
        || unsafe { GetFileType(handle) } != FILE_TYPE_DISK
    {
        return Err(PrivateHandleError::Security);
    }
    validate_security(handle)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AceKind {
    Allow,
    Deny,
    Unsupported,
}

fn evaluate_ace(kind: AceKind, inherit_only: bool, trusted: bool) -> Result<(), ()> {
    if inherit_only {
        return Ok(());
    }
    match (kind, trusted) {
        (AceKind::Allow, true) | (AceKind::Deny, _) => Ok(()),
        _ => Err(()),
    }
}

fn validate_security(handle: HANDLE) -> Result<(), PrivateHandleError> {
    let current_sid_storage = current_user_sid()?;
    let current_sid = unsafe {
        (*(current_sid_storage.as_ptr() as *const windows_sys::Win32::Security::TOKEN_USER))
            .User
            .Sid
    };

    let mut owner: PSID = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    // Owner and DACL pointers borrow this allocation for the checks below.
    let _descriptor_guard = SecurityDescriptorGuard(descriptor);
    if status != ERROR_SUCCESS
        || owner.is_null()
        || dacl.is_null()
        || unsafe { IsValidSid(owner) } == 0
        || unsafe { IsValidAcl(dacl) } == 0
        || unsafe { windows_sys::Win32::Security::EqualSid(owner, current_sid) } == 0
    {
        return Err(PrivateHandleError::Security);
    }

    let mut size = ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            dacl,
            &mut size as *mut _ as *mut c_void,
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            windows_sys::Win32::Security::AclSizeInformation,
        )
    } == 0
        || size.AceCount == 0
        || size.AclBytesInUse < size_of::<ACL>() as u32
        || size.AclBytesInUse > unsafe { (*dacl).AclSize } as u32
    {
        return Err(PrivateHandleError::Security);
    }

    let acl_start = dacl as usize;
    let acl_bytes = size.AclBytesInUse as usize;
    let mut trusted_allow = false;
    for index in 0..size.AceCount {
        let mut raw: *mut c_void = null_mut();
        if unsafe { GetAce(dacl, index, &mut raw) } == 0 || raw.is_null() {
            return Err(PrivateHandleError::Security);
        }
        let ace_start = raw as usize;
        let offset = ace_start
            .checked_sub(acl_start)
            .ok_or(PrivateHandleError::Security)?;
        if offset
            .checked_add(size_of::<ACE_HEADER>())
            .map_or(true, |end| end > acl_bytes)
        {
            return Err(PrivateHandleError::Security);
        }
        let header = unsafe { *(raw as *const ACE_HEADER) };
        let ace_bytes = header.AceSize as usize;
        if ace_bytes < size_of::<ACE_HEADER>()
            || offset
                .checked_add(ace_bytes)
                .map_or(true, |end| end > acl_bytes)
        {
            return Err(PrivateHandleError::Security);
        }
        let kind = match header.AceType {
            value
                if value
                    == windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE
                        as u8 =>
            {
                AceKind::Allow
            }
            value
                if value
                    == windows_sys::Win32::System::SystemServices::ACCESS_DENIED_ACE_TYPE as u8 =>
            {
                AceKind::Deny
            }
            _ => AceKind::Unsupported,
        };
        let inherit_only = header.AceFlags & INHERIT_ONLY_ACE as u8 != 0;
        if kind == AceKind::Unsupported && inherit_only {
            continue;
        }
        let trusted = if kind == AceKind::Unsupported {
            false
        } else {
            let sid = ace_sid(raw, ace_bytes).ok_or(PrivateHandleError::Security)?;
            let trusted = sid_is_allowed(sid, current_sid);
            if kind == AceKind::Allow && trusted && !inherit_only {
                trusted_allow = true;
            }
            trusted
        };
        if evaluate_ace(kind, inherit_only, trusted).is_err() {
            return Err(PrivateHandleError::Security);
        }
    }
    if !trusted_allow {
        return Err(PrivateHandleError::Security);
    }
    Ok(())
}

fn ace_sid(raw: *mut c_void, ace_bytes: usize) -> Option<PSID> {
    const SID_START_OFFSET: usize = size_of::<ACE_HEADER>() + size_of::<u32>();
    const MIN_SID_BYTES: usize = 8;
    if SID_START_OFFSET
        .checked_add(MIN_SID_BYTES)
        .map_or(true, |minimum| ace_bytes < minimum)
    {
        return None;
    }
    let sid_address = (raw as usize).checked_add(SID_START_OFFSET)?;
    let sid = sid_address as PSID;
    let subauthority_count = unsafe { *(sid_address.checked_add(1)? as *const u8) } as usize;
    let sid_bytes = MIN_SID_BYTES.checked_add(subauthority_count.checked_mul(4)?)?;
    if SID_START_OFFSET
        .checked_add(sid_bytes)
        .map_or(true, |end| end > ace_bytes)
        || unsafe { IsValidSid(sid) } == 0
        || unsafe { windows_sys::Win32::Security::GetLengthSid(sid) } as usize != sid_bytes
    {
        return None;
    }
    Some(sid)
}

fn sid_is_allowed(sid: PSID, current: PSID) -> bool {
    if unsafe { windows_sys::Win32::Security::EqualSid(sid, current) } != 0 {
        return true;
    }
    const SID_STORAGE_WORDS: usize =
        (windows_sys::Win32::Security::SECURITY_MAX_SID_SIZE as usize).div_ceil(size_of::<usize>());
    for kind in [
        windows_sys::Win32::Security::WinLocalSystemSid,
        windows_sys::Win32::Security::WinBuiltinAdministratorsSid,
    ] {
        let mut storage = [0usize; SID_STORAGE_WORDS];
        let mut length = windows_sys::Win32::Security::SECURITY_MAX_SID_SIZE;
        if unsafe {
            windows_sys::Win32::Security::CreateWellKnownSid(
                kind,
                null_mut(),
                storage.as_mut_ptr() as PSID,
                &mut length,
            )
        } != 0
            && unsafe { windows_sys::Win32::Security::EqualSid(sid, storage.as_mut_ptr() as PSID) }
                != 0
        {
            return true;
        }
    }
    false
}

fn current_user_sid() -> Result<Vec<usize>, PrivateHandleError> {
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TokenUser};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut raw_token: HANDLE = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) } == 0 {
        return Err(PrivateHandleError::Security);
    }
    let token = WinHandle(raw_token);
    let mut required = 0u32;
    unsafe {
        GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut required);
    }
    if required < size_of::<windows_sys::Win32::Security::TOKEN_USER>() as u32 {
        return Err(PrivateHandleError::Security);
    }
    let words = (required as usize).div_ceil(size_of::<usize>());
    let mut storage = vec![0usize; words];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            storage.as_mut_ptr() as *mut c_void,
            required,
            &mut required,
        )
    } == 0
    {
        return Err(PrivateHandleError::Security);
    }
    let sid = unsafe {
        (*(storage.as_ptr() as *const windows_sys::Win32::Security::TOKEN_USER))
            .User
            .Sid
    };
    if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Err(PrivateHandleError::Security);
    }
    Ok(storage)
}

struct WinHandle(HANDLE);

impl Drop for WinHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.0);
            }
        }
    }
}

struct SecurityDescriptorGuard(PSECURITY_DESCRIPTOR);

impl Drop for SecurityDescriptorGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                windows_sys::Win32::Foundation::LocalFree(
                    self.0 as windows_sys::Win32::Foundation::HLOCAL,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};

    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    };

    use super::*;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "orchester-private-handle-windows-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn system_tool(relative: &str) -> PathBuf {
        PathBuf::from(std::env::var_os("SystemRoot").unwrap())
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

    fn apply_strict_acl(path: &Path) {
        let output = Command::new(system_tool("icacls.exe"))
            .arg(path)
            .args(["/inheritance:r", "/grant:r"])
            .args([
                format!("*{}:(OI)(CI)(F)", current_sid()),
                "*S-1-5-18:(OI)(CI)(F)".to_owned(),
                "*S-1-5-32-544:(OI)(CI)(F)".to_owned(),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
    }

    fn open_directory(path: &Path) -> File {
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .unwrap()
    }

    #[test]
    fn ace_evaluator_accepts_only_trusted_non_inherited_allows() {
        assert!(evaluate_ace(AceKind::Allow, false, true).is_ok());
        assert!(evaluate_ace(AceKind::Deny, false, false).is_ok());
        assert!(evaluate_ace(AceKind::Allow, false, false).is_err());
        assert!(evaluate_ace(AceKind::Unsupported, false, true).is_err());
        assert!(evaluate_ace(AceKind::Unsupported, true, false).is_ok());
    }

    #[test]
    fn strict_owned_directory_handle_is_accepted() {
        let root = TempDir::new();
        apply_strict_acl(&root.0);
        let directory = open_directory(&root.0);

        assert_eq!(validate_private_handle(&directory, true), Ok(()));
        assert_eq!(
            validate_private_handle(&directory, false),
            Err(PrivateHandleError::Security)
        );
    }
}
