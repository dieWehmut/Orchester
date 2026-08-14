use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, HANDLE};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    GetAce, GetAclInformation, IsValidAcl, IsValidSid, ACE_HEADER, ACL, ACL_SIZE_INFORMATION,
    DACL_SECURITY_INFORMATION, INHERIT_ONLY_ACE, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    PSID,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileInformationByHandle, GetFileType, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_TYPE_DISK,
};

use super::PrivateHandleError;

/// Create `path` and any missing ancestor, breaking ACL inheritance on each
/// directory we create so it grants only this user, SYSTEM, and Administrators.
///
/// Directories that already exist are returned untouched: rewriting an ACL we
/// did not establish could destroy grants another principal owns, which is the
/// same reason the config loader only ever *reports* on foreign permissions.
pub(crate) fn create_private_dir_all(path: &std::path::Path) -> std::io::Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            create_private_dir_all(parent)?;
        }
    }
    match std::fs::create_dir(path) {
        Ok(()) => restrict_directory(path),
        // Losing a race against another process that created the same directory
        // is not a failure; its ACL is then not ours to rewrite.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
        Err(error) => Err(error),
    }
}

/// Write `contents` to a newly created file.
///
/// The ACL is tightened while the file is still empty, so the secret is never on
/// disk behind grants it did not choose.  Inheriting the directory's grants is
/// not enough: [`create_private_dir_all`] leaves a pre-existing directory alone,
/// so a home created before Orchester ran may carry a foreign grant that the
/// file would otherwise inherit -- and the loader would then refuse to read the
/// configuration Orchester itself had just written.
///
/// `create_new` refuses to follow a reparse point planted at `path` or to
/// truncate an existing file, which is also what makes tightening the ACL safe:
/// the file can only be one this call just created.
pub(crate) fn write_private_file(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    if let Err(error) = restrict_to_current_user(path, false) {
        // Still empty, so nothing has leaked; leaving it behind would be a trap
        // for the next writer, who would find the path already taken.
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(error);
    }
    file.write_all(contents.as_bytes())
}

/// Replace a directory's inherited ACL with an explicit user-only one.
fn restrict_directory(path: &std::path::Path) -> std::io::Result<()> {
    // (OI)(CI) propagates to anything created inside, so a subdirectory we make
    // later starts out private too.
    restrict_to_current_user(path, true).inspect_err(|_| {
        // The directory exists but is not private, so leaving it behind would be
        // a trap for the secret about to be written into it.
        let _ = std::fs::remove_dir(path);
    })
}

/// Replace `path`'s inherited ACL with an explicit grant to this user, SYSTEM
/// and Administrators -- the same set [`sid_is_allowed`] accepts.
///
/// `icacls` is used rather than `SetNamedSecurityInfoW` because building an ACL
/// by hand is a great deal of unsafe code for a one-shot grant, and shelling out
/// to it is already how this crate inspects Windows permissions.
fn restrict_to_current_user(path: &std::path::Path, propagate: bool) -> std::io::Result<()> {
    let tool = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .map(|root| root.join("System32").join("icacls.exe"))
        .filter(|candidate| candidate.is_file())
        .ok_or_else(|| {
            std::io::Error::other(format!(
                "icacls is unavailable, so '{}' cannot be made user-only; \
                 create it manually with a user-only ACL",
                path.display()
            ))
        })?;
    let sid = current_user_sid_string()?;
    let grant = if propagate { "(OI)(CI)(F)" } else { "(F)" };
    let output = std::process::Command::new(tool)
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .args([
            format!("*{sid}:{grant}"),
            format!("*S-1-5-18:{grant}"),
            format!("*S-1-5-32-544:{grant}"),
        ])
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "could not restrict '{}' to the current user",
        path.display()
    )))
}

/// `icacls /grant:r` replaces grants for the named SIDs but preserves other
/// explicit ACEs. Remove every non-trusted allow ACE after inheritance is
/// broken so a file cannot retain a broad grant from an older ACL.
fn remove_untrusted_grants(path: &std::path::Path) -> std::io::Result<()> {
    let powershell = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .map(|root| {
            root.join("System32")
                .join(r"WindowsPowerShell\v1.0\powershell.exe")
        })
        .filter(|candidate| candidate.is_file())
        .ok_or_else(|| std::io::Error::other("Windows PowerShell is unavailable"))?;
    let icacls = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .map(|root| root.join("System32").join("icacls.exe"))
        .filter(|candidate| candidate.is_file())
        .ok_or_else(|| std::io::Error::other("icacls is unavailable"))?;
    const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$trusted = @(
  [Security.Principal.WindowsIdentity]::GetCurrent().User.Value,
  'S-1-5-18',
  'S-1-5-32-544'
)
$untrusted = @(
  foreach ($ace in (Get-Acl -LiteralPath $env:ORCHESTER_RESTRICT_PATH).Access) {
    if ($ace.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow) { continue }
    $sid = $ace.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value
    if ($trusted -notcontains $sid) { "*$sid" }
  }
) | Sort-Object -Unique
if ($untrusted.Count -gt 0) {
  & $env:ORCHESTER_RESTRICT_ICACLS $env:ORCHESTER_RESTRICT_PATH '/remove:g' $untrusted | Out-Null
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
"#;
    let output = std::process::Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            SCRIPT,
        ])
        .env("ORCHESTER_RESTRICT_PATH", path)
        .env("ORCHESTER_RESTRICT_ICACLS", icacls)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "could not remove untrusted file grants",
        ))
    }
}

pub(crate) fn restrict_private_file(path: &std::path::Path) -> std::io::Result<()> {
    restrict_to_current_user(path, false)?;
    remove_untrusted_grants(path)
}

/// The current user's SID in `S-1-5-…` form, the unambiguous way to name a
/// principal to `icacls`: an account name can be shadowed across domains.
fn current_user_sid_string() -> std::io::Result<String> {
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;

    let storage = current_user_sid()
        .map_err(|_| std::io::Error::other("could not read the current user identity"))?;
    let sid = unsafe {
        (*(storage.as_ptr() as *const windows_sys::Win32::Security::TOKEN_USER))
            .User
            .Sid
    };
    let mut raw: *mut u16 = null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut raw) } == 0 || raw.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let _guard = LocalStringGuard(raw);
    let mut length = 0usize;
    while unsafe { *raw.add(length) } != 0 {
        length += 1;
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(raw, length) })
        .map_err(|_| std::io::Error::other("the current user identity is not valid UTF-16"))
}

struct LocalStringGuard(*mut u16);

impl Drop for LocalStringGuard {
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

pub(crate) fn validate_private_handle(
    file: &File,
    expect_directory: bool,
) -> Result<(), PrivateHandleError> {
    let handle = validate_handle_kind(file, expect_directory)?;
    validate_owner(handle)?;
    validate_dacl(handle)
}

pub(crate) fn validate_private_handle_identity(
    file: &File,
    expect_directory: bool,
) -> Result<(), PrivateHandleError> {
    let handle = validate_handle_kind(file, expect_directory)?;
    validate_owner(handle)
}

fn validate_handle_kind(file: &File, expect_directory: bool) -> Result<HANDLE, PrivateHandleError> {
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
    Ok(handle)
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

fn validate_owner(handle: HANDLE) -> Result<(), PrivateHandleError> {
    let current_sid_storage = current_user_sid()?;
    let current_sid = unsafe {
        (*(current_sid_storage.as_ptr() as *const windows_sys::Win32::Security::TOKEN_USER))
            .User
            .Sid
    };

    let mut owner: PSID = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut descriptor,
        )
    };
    let _descriptor_guard = SecurityDescriptorGuard(descriptor);
    if status != ERROR_SUCCESS
        || owner.is_null()
        || unsafe { IsValidSid(owner) } == 0
        || unsafe { windows_sys::Win32::Security::EqualSid(owner, current_sid) } == 0
    {
        return Err(PrivateHandleError::Security);
    }
    Ok(())
}

fn validate_dacl(handle: HANDLE) -> Result<(), PrivateHandleError> {
    let current_sid_storage = current_user_sid()?;
    let current_sid = unsafe {
        (*(current_sid_storage.as_ptr() as *const windows_sys::Win32::Security::TOKEN_USER))
            .User
            .Sid
    };

    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    let _descriptor_guard = SecurityDescriptorGuard(descriptor);
    if status != ERROR_SUCCESS || dacl.is_null() || unsafe { IsValidAcl(dacl) } == 0 {
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
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY};
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;

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

    fn open_file(path: &Path) -> File {
        OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .unwrap()
    }

    /// `BUILTIN\Users` stands in for the sandbox group a host may already have
    /// granted on the Orchester home; `(OI)(CI)` is what makes it reach files
    /// created inside afterwards.
    fn add_inheritable_read_grant(directory: &Path, sid: &str) {
        let output = Command::new(system_tool("icacls.exe"))
            .arg(directory)
            .args(["/grant", &format!("*{sid}:(OI)(CI)(RX)")])
            .output()
            .unwrap();
        assert!(output.status.success());
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

    /// A pre-existing Orchester home may already carry a foreign grant, and
    /// `create_private_dir_all` deliberately leaves such a directory alone.  A
    /// file written into it must still be private on its own, the way the unix
    /// implementation gets for free from `mode(0o600)` at creation -- otherwise
    /// `/login` writes a configuration that the loader then refuses to read.
    #[test]
    fn a_file_written_into_a_shared_directory_is_still_private() {
        let root = TempDir::new();
        apply_strict_acl(&root.0);
        add_inheritable_read_grant(&root.0, "S-1-5-32-545");
        let path = root.0.join("orchester.jsonc");

        write_private_file(&path, "{}").unwrap();

        assert_eq!(validate_private_handle(&open_file(&path), false), Ok(()));
    }
}
