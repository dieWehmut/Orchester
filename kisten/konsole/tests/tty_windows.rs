#![cfg(windows)]

#[path = "support/conpty.rs"]
mod conpty;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use conpty::ConPty;

const READY_TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn conpty_captures_a_native_console_process() {
    let workspace = temp_home("conpty-native-workspace");
    std::fs::create_dir_all(&workspace).expect("create native ConPTY workspace");
    let system_root = std::env::var_os("SystemRoot").expect("SystemRoot");
    let command = Path::new(&system_root).join("System32").join("cmd.exe");
    let mut session =
        ConPty::spawn(&command, &workspace, &[], 80, 24).expect("spawn cmd.exe in ConPTY");

    session
        .write(b"echo ORCHESTER_CONPTY_READY\r\n")
        .expect("drive native console process");
    session
        .read_until(b"ORCHESTER_CONPTY_READY", READY_TIMEOUT)
        .expect("native console output");
    session
        .write(b"exit\r\n")
        .expect("exit native console process");
    let (exit_code, _) = session
        .wait_for_exit(READY_TIMEOUT)
        .expect("native console process exits");

    assert_eq!(exit_code, 0);
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn status_overlay_keeps_one_full_screen_session_and_stable_header() {
    let home = temp_home("conpty-home");
    let workspace = temp_home("conpty-workspace");
    std::fs::create_dir_all(&home).expect("create ConPTY home");
    std::fs::create_dir_all(&workspace).expect("create ConPTY workspace");
    let mut session = ConPty::spawn(
        Path::new(env!("CARGO_BIN_EXE_orchester")),
        &workspace,
        &[(
            OsString::from("ORCHESTER_HOME"),
            home.as_os_str().to_os_string(),
        )],
        120,
        40,
    )
    .expect("spawn Orchester in ConPTY");

    let initial_end = session
        .read_until(b">_ Orchester", READY_TIMEOUT)
        .expect("initial workspace panel");
    session.write(b"/status\r").expect("open status overlay");
    let overlay_end = session
        .read_until_since(initial_end, b"Self-agent status", READY_TIMEOUT)
        .expect("status overlay");
    session.write(b"\x1b").expect("close status overlay");
    let home_end = session
        .read_until_since(overlay_end, b"\x1b[?2026h", READY_TIMEOUT)
        .expect("home frame committed");
    session.write(b"/quit\r").expect("quit TUI");
    let (exit_code, output) = session
        .wait_for_exit(READY_TIMEOUT)
        .expect("Orchester exits after /quit");

    assert_eq!(exit_code, 0);
    assert_eq!(count(&output, b"\x1b[?1049h"), 1, "alternate screen enter");
    assert_eq!(count(&output, b"\x1b[?1049l"), 1, "alternate screen leave");
    assert_eq!(count(&output, b">_ Orchester"), 1, "stable top panel");
    assert!(contains(&output, b"Self-agent status"));
    assert!(
        !contains(&output[initial_end..home_end], b"\x1b[2J"),
        "interactive frame updates must not clear the full screen"
    );

    let _ = std::fs::remove_dir_all(home);
    let _ = std::fs::remove_dir_all(workspace);
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn temp_home(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "orchester-cli-{name}-{}-{nanos}",
        std::process::id()
    ))
}
