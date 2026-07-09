use std::io::Write;
use std::process::{Command, Stdio};

fn orchester() -> Command {
    Command::new(env!("CARGO_BIN_EXE_orchester"))
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn json_events(output: &std::process::Output) -> Vec<serde_json::Value> {
    stdout(output)
        .lines()
        .map(|line| serde_json::from_str(line).expect("valid Event JSONL"))
        .collect()
}

#[test]
fn list_shows_builtin_adapters() {
    let output = orchester()
        .arg("list")
        .output()
        .expect("run orchester list");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    for name in ["claude", "codex", "mock", "opencode"] {
        assert!(out.contains(name), "missing adapter {name} in:\n{out}");
    }
}

#[test]
fn doctor_reports_mock_adapter_available() {
    let output = orchester()
        .arg("doctor")
        .output()
        .expect("run orchester doctor");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("mock"), "doctor output:\n{out}");
    assert!(out.contains("ok"), "doctor output:\n{out}");
    assert!(
        out.contains("built-in mock adapter"),
        "doctor output:\n{out}"
    );
}

#[test]
fn default_run_can_emit_event_jsonl() {
    let output = orchester()
        .args(["--agent", "mock", "--json", "hello default"])
        .output()
        .expect("run mock agent");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let events = json_events(&output);
    assert_eq!(events.first().unwrap()["type"], "session_started");
    assert!(events.iter().any(|event| event["type"] == "message"
        && event["text"].as_str().unwrap().contains("hello default")));
    assert!(events.iter().any(|event| event["type"] == "result"
        && event["text"].as_str().unwrap().contains("hello default")));
}

#[test]
fn run_subcommand_can_emit_event_jsonl() {
    let output = orchester()
        .args(["run", "--agent", "mock", "--json", "hello run"])
        .output()
        .expect("run mock agent through run subcommand");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let events = json_events(&output);
    assert_eq!(events.first().unwrap()["type"], "session_started");
    assert!(events
        .iter()
        .any(|event| event["type"] == "result"
            && event["text"].as_str().unwrap().contains("hello run")));
}

#[test]
fn run_subcommand_reads_prompt_from_stdin() {
    let mut child = orchester()
        .args(["run", "--agent", "mock", "--json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn orchester");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"hello stdin\n")
        .expect("write prompt");

    let output = child.wait_with_output().expect("collect output");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let events = json_events(&output);
    assert!(events.iter().any(|event| event["type"] == "message"
        && event["text"].as_str().unwrap().contains("hello stdin")));
}
