use std::path::{Path, PathBuf};

/// Write a user configuration into an Orchester home, with the private
/// permissions the loader insists on.
///
/// `home` is the Orchester home itself — the directory `ORCHESTER_HOME` names —
/// not the surrounding user home, so the file lands exactly where the loader
/// looks for it.
pub fn write_user_config(home: &Path, source: &str) -> PathBuf {
    std::fs::create_dir_all(home).expect("create config directory");
    let file = home.join("orchester.jsonc");
    std::fs::write(&file, source).expect("write user config");
    make_permissions_secure(home, &file);
    file
}

#[cfg(unix)]
fn make_permissions_secure(directory: &Path, file: &Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
        .expect("secure config directory");
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600))
        .expect("secure config file");
}

#[cfg(windows)]
fn make_permissions_secure(directory: &Path, file: &Path) {
    use std::process::Command;

    fn system_tool(relative: &str) -> PathBuf {
        PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot"))
            .join("System32")
            .join(relative)
    }

    let identity = Command::new(system_tool("WindowsPowerShell\\v1.0\\powershell.exe"))
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value",
        ])
        .output()
        .expect("resolve current SID");
    assert!(identity.status.success(), "resolve current SID");
    let sid = String::from_utf8(identity.stdout)
        .expect("SID text")
        .trim()
        .to_owned();
    let icacls = system_tool("icacls.exe");
    for (path, grants) in [
        (
            directory,
            [
                format!("*{sid}:(OI)(CI)(F)"),
                "*S-1-5-18:(OI)(CI)(F)".to_owned(),
                "*S-1-5-32-544:(OI)(CI)(F)".to_owned(),
            ],
        ),
        (
            file,
            [
                format!("*{sid}:(F)"),
                "*S-1-5-18:(F)".to_owned(),
                "*S-1-5-32-544:(F)".to_owned(),
            ],
        ),
    ] {
        let output = Command::new(&icacls)
            .arg(path)
            .args(["/inheritance:r", "/grant:r"])
            .args(grants)
            .output()
            .expect("apply strict config ACL");
        assert!(output.status.success(), "apply strict config ACL");
    }
}

#[cfg(not(any(unix, windows)))]
fn make_permissions_secure(_directory: &Path, _file: &Path) {}
