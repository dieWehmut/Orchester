#![cfg(windows)]

#[path = "support/conpty.rs"]
mod conpty;
#[path = "support/loopback_responses.rs"]
mod loopback_responses;
#[path = "support/secure_config.rs"]
mod secure_config;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use conpty::ConPty;
use loopback_responses::LoopbackResponses;
use secure_config::write_user_config;

const READY_TIMEOUT: Duration = Duration::from_secs(10);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

static CONPTY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn conpty_test_guard() -> std::sync::MutexGuard<'static, ()> {
    CONPTY_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn conpty_captures_a_native_console_process() {
    let _guard = conpty_test_guard();
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
    let _guard = conpty_test_guard();
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

#[test]
fn command_pickers_share_one_stable_full_screen_session() {
    let _guard = conpty_test_guard();
    let home = temp_home("conpty-command-pickers-home");
    let workspace = temp_home("conpty-command-pickers-workspace");
    std::fs::create_dir_all(&home).expect("create command-picker ConPTY home");
    std::fs::create_dir_all(&workspace).expect("create command-picker ConPTY workspace");
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
    let mut cursor = initial_end;
    for (command, title, footer) in [
        ("/permissions", "Self-agent permissions", "Enter inspect"),
        ("/model", "Select model", "Enter choose effort"),
        (
            "/resume",
            "Resumable self-agent runs",
            "Enter resume/inspect",
        ),
        ("/status", "Self-agent status", "Enter inspect"),
        ("/config", "Self-agent configuration", "Enter inspect"),
        ("/plugins", "Installed agent plugins", "Enter inspect"),
        ("/theme", "Select theme", "Enter save"),
    ] {
        session
            .write(format!("{command}\r").as_bytes())
            .expect("open command picker");
        cursor = session
            .read_until_since(cursor, footer.as_bytes(), COMMAND_TIMEOUT)
            .unwrap_or_else(|error| {
                panic!("{command} picker {title:?} was not selectable: {error}")
            });
        session.write(b"\x1b").expect("close command picker");
        cursor = session
            .read_until_since(cursor, b"\x1b[?2026h", COMMAND_TIMEOUT)
            .unwrap_or_else(|error| panic!("{command} did not restore the home frame: {error}"));
    }
    session.write(b"/quit\r").expect("quit TUI");
    let (exit_code, output) = session
        .wait_for_exit(READY_TIMEOUT)
        .expect("Orchester exits after command-picker smoke test");

    assert_eq!(exit_code, 0);
    assert_eq!(count(&output, b"\x1b[?1049h"), 1, "alternate screen enter");
    assert_eq!(count(&output, b"\x1b[?1049l"), 1, "alternate screen leave");
    assert_eq!(count(&output, b">_ Orchester"), 1, "stable top panel");
    for title in [
        "Self-agent permissions",
        "Select model",
        "Resumable self-agent runs",
        "Self-agent status",
        "Self-agent configuration",
        "Installed agent plugins",
        "Select theme",
    ] {
        assert!(
            contains(&output, title.as_bytes()),
            "command picker title did not render: {title}"
        );
    }
    assert!(
        !contains(&output[initial_end..], b"\x1b[2J"),
        "command picker transitions must not clear the full screen"
    );

    let _ = std::fs::remove_dir_all(home);
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn composer_edits_commands_and_keeps_multiline_paste_as_a_draft() {
    let _guard = conpty_test_guard();
    let home = temp_home("conpty-editor-home");
    let workspace = temp_home("conpty-editor-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let mut session = ConPty::spawn(
        Path::new(env!("CARGO_BIN_EXE_orchester")),
        &workspace,
        &[(
            OsString::from("ORCHESTER_HOME"),
            home.as_os_str().to_os_string(),
        )],
        80,
        24,
    )
    .unwrap();
    let mut cursor = session.read_until(b">_ Orchester", READY_TIMEOUT).unwrap();
    session.write(b"/sttus\x1b[D\x1b[D\x1b[Da\r").unwrap();
    cursor = session
        .read_until_since(cursor, b"Self-agent status", READY_TIMEOUT)
        .unwrap();
    session.write(b"\x1b").unwrap();
    cursor = session
        .read_until_since(cursor, b"Queued: /status", READY_TIMEOUT)
        .unwrap();
    session
        .write(b"\x1b[200~/quit\r\nsecond-line-draft\x1b[201~")
        .unwrap();
    session
        .read_until_since(cursor, b"second-line-draft", READY_TIMEOUT)
        .expect("bracketed multiline paste must remain in the draft, without executing /quit");
    session.write(b"\x15/quit\r").unwrap();
    let (code, output) = session.wait_for_exit(READY_TIMEOUT).unwrap();
    assert_eq!(code, 0);
    assert_eq!(count(&output, b"\x1b[?1049h"), 1);
    assert_eq!(count(&output, b"\x1b[?1049l"), 1);
    assert!(
        !home.join("state/runs.db").exists(),
        "editing or pasting must not create a run"
    );
    let _ = std::fs::remove_dir_all(home);
    let _ = std::fs::remove_dir_all(workspace);
}

#[test]
fn resized_terminal_submits_one_multiline_prompt_then_recalls_it_for_another_turn() {
    let _guard = conpty_test_guard();
    let home = temp_home("conpty-conversation-home");
    let workspace = temp_home("conpty-conversation-workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let answer = format!(
        "{}\nFIRST_OK",
        "检查完成：中文、emoji 👩‍💻 和长回复会保留全部内容。 ".repeat(8)
    );
    let server = LoopbackResponses::start(vec![
        serde_json::json!({ "status": "completed", "output": [{ "type": "message", "role": "assistant",
            "content": [{ "type": "output_text", "text": answer }] }] }),
        serde_json::json!({ "status": "completed", "output": [{ "type": "message", "role": "assistant",
            "content": [{ "type": "output_text", "text": "SECOND_OK" }] }] }),
    ]);
    write_user_config(&home, &serde_json::json!({
        "model_provider": "Loopback", "model": "gpt-loopback", "disable_response_storage": true,
        "model_providers": { "Loopback": { "base_url": server.base_url(),
            "api_key": "conpty-model-secret-canary", "wire_api": "responses", "requires_openai_auth": true } }
    }).to_string());
    let mut session = ConPty::spawn(
        Path::new(env!("CARGO_BIN_EXE_orchester")),
        &workspace,
        &[
            (
                OsString::from("ORCHESTER_HOME"),
                home.as_os_str().to_os_string(),
            ),
            (
                OsString::from("NO_PROXY"),
                OsString::from("127.0.0.1,localhost"),
            ),
            (
                OsString::from("no_proxy"),
                OsString::from("127.0.0.1,localhost"),
            ),
        ],
        80,
        24,
    )
    .unwrap();
    let mut cursor = session.read_until(b">_ Orchester", READY_TIMEOUT).unwrap();
    capture_terminal(&mut session, "home-80x24.ansi");
    let prompt = "检查工作区 👩‍💻\n请保留完整回答";
    session
        .write(format!("\x1b[200~{}\x1b[201~", prompt.replace('\n', "\r\n")).as_bytes())
        .unwrap();
    cursor = session
        .read_until_since(cursor, "请保留完整回答".as_bytes(), READY_TIMEOUT)
        .unwrap();
    capture_terminal(&mut session, "multiline-draft-80x24.ansi");
    session.write(b"\r").unwrap();
    cursor = session
        .read_until_since(cursor, b"FIRST_OK", COMMAND_TIMEOUT)
        .unwrap();
    capture_terminal(&mut session, "conversation-80x24.ansi");
    if let Some(directory) = std::env::var_os("ORCHESTER_CLI_CAPTURE_DIR") {
        let offset = session.snapshot().unwrap().len();
        std::fs::write(
            PathBuf::from(directory).join("terminal-resize.json"),
            serde_json::json!({"offset": offset, "columns": 40, "rows": 12}).to_string(),
        )
        .unwrap();
    }
    session.resize(40, 12).unwrap();
    session.write(b"\x1b[5~\x1b[6~").unwrap();
    cursor = session
        .read_until_since(cursor, b"\x1b[?2026h", READY_TIMEOUT)
        .unwrap();
    // Up recalls at the top of the empty composer; Ctrl+U then replaces that
    // draft. Neither navigation nor resizing may create an extra model turn.
    session.write(b"\x1b[A\x15second prompt\r").unwrap();
    session
        .read_until_since(cursor, b"SECOND_OK", COMMAND_TIMEOUT)
        .unwrap();
    capture_terminal(&mut session, "conversation-40x12.ansi");
    session.write(b"/quit\r").unwrap();
    let (code, output) = session.wait_for_exit(READY_TIMEOUT).unwrap();
    assert_eq!(code, 0);
    assert_eq!(count(&output, b"\x1b[?1049h"), 1);
    assert_eq!(count(&output, b"\x1b[?1049l"), 1);
    assert!(!contains(&output, b"conpty-model-secret-canary"));
    let requests = server.finish();
    assert_eq!(
        requests.len(),
        2,
        "paste, resize and recall must not submit extra requests"
    );
    let request = &requests[0];
    let body_start = request
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap()
        + 4;
    let body: serde_json::Value = serde_json::from_slice(&request[body_start..]).unwrap();
    let sent_prompt = body["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "user")
        .and_then(|message| message["content"][0]["text"].as_str())
        .unwrap();
    assert_eq!(
        sent_prompt, prompt,
        "the exact multiline prompt must reach the provider"
    );
    let _ = std::fs::remove_dir_all(home);
    let _ = std::fs::remove_dir_all(workspace);
}

fn capture_terminal(session: &mut ConPty, name: &str) {
    if let Some(directory) = std::env::var_os("ORCHESTER_CLI_CAPTURE_DIR") {
        let directory = PathBuf::from(directory);
        assert!(
            directory.is_absolute(),
            "capture directory must be absolute"
        );
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(name), session.snapshot().unwrap()).unwrap();
    }
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
