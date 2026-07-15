use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

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

fn temp_home(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "orchester-cli-{name}-{}-{nanos}",
        std::process::id()
    ))
}

fn install_repository_plugin(scope: &Path, marker: &str) {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("npm/plugins/claude");
    let destination = scope.join("claude");
    std::fs::create_dir_all(destination.join("manifests")).unwrap();
    for relative in ["package.json", "orchester-plugin.json"] {
        std::fs::copy(source.join(relative), destination.join(relative)).unwrap();
    }
    let manifest = std::fs::read_to_string(source.join("manifests/claude.toml"))
        .unwrap()
        .replace(
            "kinds = [\"code\", \"chat\"]",
            &format!("kinds = [\"{marker}\"]"),
        );
    std::fs::write(destination.join("manifests/claude.toml"), manifest).unwrap();
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
fn list_can_emit_capability_jsonl() {
    let output = orchester()
        .args(["list", "--json"])
        .output()
        .expect("run orchester list --json");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let values: Vec<serde_json::Value> = stdout(&output)
        .lines()
        .map(|line| serde_json::from_str(line).expect("valid capability JSONL"))
        .collect();

    assert!(values.iter().any(|value| value["name"] == "mock"));
    assert!(
        values
            .iter()
            .any(|value| value["name"] == "mock" && value["streaming"] == true)
    );
}

#[test]
fn list_discovers_project_npm_plugins() {
    let project = temp_home("project-plugin");
    let home = temp_home("project-plugin-home");
    std::fs::create_dir_all(&project).unwrap();
    install_repository_plugin(
        &project.join("node_modules/@orchester"),
        "project-plugin-marker",
    );

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", &home)
        .arg("list")
        .output()
        .expect("list project plugin adapters");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("project-plugin-marker"));
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn list_discovers_managed_npm_plugins() {
    let project = temp_home("managed-plugin-project");
    let home = temp_home("managed-plugin-home");
    std::fs::create_dir_all(&project).unwrap();
    install_repository_plugin(
        &home.join("plugins/npm/node_modules/@orchester"),
        "managed-plugin-marker",
    );

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", &home)
        .arg("list")
        .output()
        .expect("list managed plugin adapters");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("managed-plugin-marker"));
    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn relative_orchester_home_fails_without_echoing_the_value() {
    let project = temp_home("relative-home-project");
    std::fs::create_dir_all(&project).unwrap();

    let output = orchester()
        .current_dir(&project)
        .env("ORCHESTER_HOME", "relative-secret-home")
        .arg("list")
        .output()
        .expect("reject relative Orchester home");

    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("managed plugin home must be an absolute path"));
    assert!(!err.contains("relative-secret-home"));
    let _ = std::fs::remove_dir_all(project);
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
    assert!(
        events.iter().any(|event| event["type"] == "result"
            && event["text"].as_str().unwrap().contains("hello run"))
    );
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

#[test]
fn no_args_can_run_interactive_mock_session() {
    let home = temp_home("interactive");
    let mut child = orchester()
        .env("ORCHESTER_HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"/agent\nmock\nhello interactive\n/quit\n")
        .expect("write interactive input");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Orchester"), "interactive output:\n{out}");
    assert!(out.contains(">_ Orchester"), "interactive output:\n{out}");
    assert!(
        out.contains("Type a task for Orchester"),
        "startup output:\n{out}"
    );
    assert!(
        out.contains("Available agents"),
        "interactive output:\n{out}"
    );
    assert!(
        out.contains("mock received: hello interactive"),
        "interactive output:\n{out}"
    );
    assert!(
        out.contains("mock done: hello interactive"),
        "interactive output:\n{out}"
    );

    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn no_args_show_home_before_launching_any_agent() {
    let home = temp_home("home");
    let mut child = orchester()
        .env("ORCHESTER_HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"/quit\n")
        .expect("write quit command");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains(">_ Orchester"), "home output:\n{out}");
    assert!(
        out.contains("Type a task for Orchester"),
        "home output:\n{out}"
    );
    assert!(
        !out.contains("Launching codex") && !out.contains("Launching claude"),
        "an agent launched before the home selection:\n{out}"
    );

    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn no_args_non_tty_requires_explicit_delegate_entrypoint() {
    let output = orchester()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run non-tty orchester");

    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Type a task for Orchester"), "output:\n{out}");
    assert!(out.contains("/agent"), "output:\n{out}");
    assert!(!out.contains("Select agent"), "output:\n{out}");
}

#[test]
fn run_records_session_metadata() {
    let home = temp_home("sessions");
    let run = orchester()
        .env("ORCHESTER_HOME", &home)
        .args(["--agent", "mock", "--json", "remember this"])
        .output()
        .expect("run mock agent");
    assert!(run.status.success(), "stderr:\n{}", stderr(&run));

    let sessions = orchester()
        .env("ORCHESTER_HOME", &home)
        .arg("sessions")
        .output()
        .expect("list sessions");
    assert!(sessions.status.success(), "stderr:\n{}", stderr(&sessions));
    let out = stdout(&sessions);
    assert!(out.contains("mock"), "sessions output:\n{out}");
    assert!(out.contains("mock-session"), "sessions output:\n{out}");
    assert!(out.contains("remember this"), "sessions output:\n{out}");

    let sessions_json = orchester()
        .env("ORCHESTER_HOME", &home)
        .args(["sessions", "--json"])
        .output()
        .expect("list sessions as json");
    assert!(
        sessions_json.status.success(),
        "stderr:\n{}",
        stderr(&sessions_json)
    );
    let value: serde_json::Value = serde_json::from_str(stdout(&sessions_json).trim()).unwrap();
    assert_eq!(value["agent"], "mock");
    assert_eq!(value["session_id"], "mock-session");
    assert_eq!(value["prompt"], "remember this");

    let _ = std::fs::remove_dir_all(home);
}
