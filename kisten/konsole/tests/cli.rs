mod support;

#[path = "support/loopback_responses.rs"]
mod loopback_responses;
#[path = "support/secure_config.rs"]
mod secure_config;

use std::io::Write;
use std::process::Stdio;

use loopback_responses::LoopbackResponses;
use secure_config::write_user_config;
use support::{orchester, stderr, stdout, temp_home};

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
    assert!(values
        .iter()
        .any(|value| value["name"] == "mock" && value["streaming"] == true));
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
fn home_prompt_enters_the_self_agent_configuration_path() {
    let home = temp_home("self-agent-unconfigured");
    std::fs::create_dir_all(&home).expect("create isolated home");
    let mut child = orchester()
        .env("ORCHESTER_HOME", &home)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");

    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"summarize recent commits\n")
        .expect("write self-agent task");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    let err = stderr(&output);
    assert!(
        err.contains("active model provider is not configured"),
        "self-agent configuration error:\n{err}"
    );
    assert!(
        !err.contains("self-agent harness is not configured yet")
            && !err.contains("enter `/agent` or `/codex`"),
        "legacy delegate placeholder was used:\n{err}"
    );

    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn status_command_reads_defaults_without_creating_run_state() {
    let home = temp_home("status-defaults");
    std::fs::create_dir_all(&home).expect("create isolated home");
    let mut child = orchester()
        .env("ORCHESTER_HOME", &home)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");
    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"/status\n")
        .expect("write status command");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Self-agent status"), "status output:\n{out}");
    assert!(
        out.contains("model: not configured"),
        "status output:\n{out}"
    );
    assert!(out.contains("state: not created"), "status output:\n{out}");
    assert!(out.contains("network ask"), "status output:\n{out}");
    assert!(out.contains("max steps 80"), "status output:\n{out}");
    assert!(
        !home.join("state/runs.db").exists(),
        "read-only status created the run database"
    );
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn permissions_command_projects_effective_policy_without_provider_or_state_access() {
    let root = temp_home("permissions-command");
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = LoopbackResponses::start(Vec::new());
    let base_url = server.base_url().to_owned();
    write_user_config(
        &home,
        &format!(
            r#"{{
                "model_provider": "Loopback",
                "model": "gpt-permissions",
                "model_providers": {{
                    "Loopback": {{
                        "base_url": "{}",
                        "api_key": "permissions-secret-canary",
                        "wire_api": "responses",
                        "requires_openai_auth": true
                    }}
                }},
                "governance": {{
                    "approval_reviewer": "user",
                    "tool_network": "deny",
                    "out_of_workspace": "allow",
                    "shell_interpreters": "allow",
                    "approval_ttl_seconds": 900
                }}
            }}"#,
            server.base_url()
        ),
    );
    let mut child = orchester()
        .current_dir(&workspace)
        .env("ORCHESTER_HOME", &home)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");
    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"/permissions\n")
        .expect("write permissions command");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");
    let requests = server.finish();

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("Self-agent permissions"),
        "permissions output:\n{out}"
    );
    assert!(
        out.contains("network: configured deny | effective deny"),
        "permissions output:\n{out}"
    );
    assert!(
        out.contains("outside: configured allow | effective deny"),
        "permissions output:\n{out}"
    );
    assert!(
        out.contains("shell: configured allow | effective deny"),
        "permissions output:\n{out}"
    );
    assert!(
        out.contains("CLI resolution: not available yet"),
        "permissions output:\n{out}"
    );
    assert!(out.contains("append-only redacted hash chain"));
    assert!(!out.contains(&base_url));
    assert!(!out.contains("permissions-secret-canary"));
    assert!(requests.is_empty(), "permission query called the provider");
    assert!(
        !home.join("state/runs.db").exists(),
        "permission query created the run database"
    );
    assert!(
        !home.join("state/audit.jsonl").exists(),
        "permission query created the audit log"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn model_command_projects_safe_configured_and_named_choices() {
    let home = temp_home("model-catalog");
    std::fs::create_dir_all(&home).expect("create isolated home");
    write_user_config(
        &home,
        r#"{
            "model_provider": "OpenAI",
            "model": "gpt-default",
            "model_reasoning_effort": "high",
            "model_providers": {
                "OpenAI": {
                    "name": "OpenAI API",
                    "base_url": "https://private-transport.example/v1",
                    "api_key": "sk-model-command-secret-canary",
                    "wire_api": "responses",
                    "requires_openai_auth": true
                }
            },
            "model_profiles": {
                "fast": {
                    "model_provider": "OpenAI",
                    "model": "gpt-fast",
                    "model_reasoning_effort": "low"
                },
                "review": {
                    "model_provider": "OpenAI",
                    "model": "gpt-review",
                    "plan_mode_reasoning_effort": "ultra"
                }
            }
        }"#,
    );
    let mut child = orchester()
        .env("ORCHESTER_HOME", &home)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");
    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"/model\n")
        .expect("write model command");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Self-agent models"), "model output:\n{out}");
    assert!(out.contains("active: configured"), "model output:\n{out}");
    assert!(out.contains("gpt-default"), "model output:\n{out}");
    assert!(
        out.contains("fast") && out.contains("gpt-fast"),
        "model output:\n{out}"
    );
    assert!(
        out.contains("review") && out.contains("gpt-review"),
        "model output:\n{out}"
    );
    for forbidden in [
        "private-transport",
        "sk-model-command-secret-canary",
        "wire_api",
        "requires_openai_auth",
    ] {
        assert!(!out.contains(forbidden), "leaked {forbidden}:\n{out}");
    }
    assert!(
        !home.join("state/runs.db").exists(),
        "read-only model query created the run database"
    );
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn model_selection_applies_to_the_next_turn_without_editing_config() {
    let root = temp_home("model-selection");
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = LoopbackResponses::start(vec![serde_json::json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "selected model response"}]
        }],
        "usage": {"input_tokens": 2, "output_tokens": 3}
    })]);
    let config = format!(
        r#"{{
            "model_provider": "Loopback",
            "model": "gpt-default",
            "disable_response_storage": true,
            "model_providers": {{
                "Loopback": {{
                    "base_url": "{}",
                    "api_key": "model-selection-secret-canary",
                    "wire_api": "responses",
                    "requires_openai_auth": true
                }}
            }},
            "model_profiles": {{
                "fast": {{
                    "model_provider": "Loopback",
                    "model": "gpt-fast",
                    "model_reasoning_effort": "low"
                }}
            }}
        }}"#,
        server.base_url()
    );
    let config_path = write_user_config(&home, &config);
    let mut child = orchester()
        .current_dir(&workspace)
        .env("ORCHESTER_HOME", &home)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");
    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"/model fast\nuse the selected model\n")
        .expect("write model selection and prompt");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");
    let requests = server.finish();

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Model selected"), "selection output:\n{out}");
    assert!(
        out.contains("fast") && out.contains("gpt-fast"),
        "selection output:\n{out}"
    );
    assert!(
        out.contains("selected model response"),
        "model output:\n{out}"
    );
    assert!(!out.contains("model-selection-secret-canary"));
    assert_eq!(requests.len(), 1, "expected one selected-model request");
    let request = String::from_utf8_lossy(&requests[0]);
    assert!(
        request.contains("\"model\":\"gpt-fast\""),
        "request:\n{request}"
    );
    assert!(
        request.contains("use the selected model"),
        "request:\n{request}"
    );
    assert_eq!(
        std::fs::read_to_string(config_path).expect("read unchanged config"),
        config,
        "session selection edited the user configuration"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn configured_model_restores_the_file_default_before_the_next_turn() {
    let root = temp_home("model-configured");
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let server = LoopbackResponses::start(vec![serde_json::json!({
        "status": "completed",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "configured model response"}]
        }],
        "usage": {"input_tokens": 2, "output_tokens": 3}
    })]);
    let config = format!(
        r#"{{
            "model_provider": "Loopback",
            "model": "gpt-default",
            "disable_response_storage": true,
            "model_providers": {{
                "Loopback": {{
                    "base_url": "{}",
                    "api_key": "model-configured-secret-canary",
                    "wire_api": "responses",
                    "requires_openai_auth": true
                }}
            }},
            "model_profiles": {{
                "fast": {{
                    "model_provider": "Loopback",
                    "model": "gpt-fast"
                }}
            }}
        }}"#,
        server.base_url()
    );
    let config_path = write_user_config(&home, &config);
    let mut child = orchester()
        .current_dir(&workspace)
        .env("ORCHESTER_HOME", &home)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");
    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"/model fast\n/model configured\nuse the configured model\n")
        .expect("write model selections and prompt");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");
    let requests = server.finish();

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert_eq!(out.matches("Model selected").count(), 2, "output:\n{out}");
    assert!(out.contains("Model selected") && out.contains("configured"));
    assert!(out.contains("configured model response"), "output:\n{out}");
    assert!(!out.contains("model-configured-secret-canary"));
    assert_eq!(requests.len(), 1, "expected one configured-model request");
    let request = String::from_utf8_lossy(&requests[0]);
    assert!(
        request.contains("\"model\":\"gpt-default\""),
        "request:\n{request}"
    );
    assert!(
        request.contains("use the configured model"),
        "request:\n{request}"
    );
    assert_eq!(
        std::fs::read_to_string(config_path).expect("read unchanged config"),
        config,
        "configured selection edited the user configuration"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn home_prompt_runs_governed_tools_until_the_model_returns_text() {
    let root = temp_home("self-agent-loop");
    let home = root.join("home");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(workspace.join("src")).expect("create workspace");
    std::fs::write(
        workspace.join("src/lib.rs"),
        "pub const LOOPBACK_VALUE: u8 = 7;\n",
    )
    .expect("write workspace fixture");
    let server = LoopbackResponses::start(vec![
        serde_json::json!({
            "status": "completed",
            "output": [{
                "type": "function_call",
                "call_id": "provider-call-read",
                "name": "read_file",
                "arguments": "{\"path\":\"src/lib.rs\",\"start_line\":null,\"end_line\":null}",
                "status": "completed"
            }],
            "usage": {"input_tokens": 3, "output_tokens": 5}
        }),
        serde_json::json!({
            "status": "completed",
            "output": [{
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "loopback inspection complete"}]
            }],
            "usage": {"input_tokens": 7, "output_tokens": 11}
        }),
    ]);
    let config = format!(
        r#"{{
            "model_provider": "Loopback",
            "model": "gpt-loopback",
            "disable_response_storage": true,
            "model_providers": {{
                "Loopback": {{
                    "base_url": "{}",
                    "api_key": "loopback-provider-secret-canary",
                    "wire_api": "responses",
                    "requires_openai_auth": true
                }}
            }},
            "limits": {{"max_steps": 4, "max_observation_bytes": 65536}}
        }}"#,
        server.base_url()
    );
    write_user_config(&home, &config);
    let mut child = orchester()
        .current_dir(&workspace)
        .env("ORCHESTER_HOME", &home)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("no_proxy", "127.0.0.1,localhost")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interactive orchester");
    child
        .stdin
        .as_mut()
        .expect("stdin handle")
        .write_all(b"inspect the source\n")
        .expect("write self-agent task");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect output");
    let requests = server.finish();
    assert!(
        output.status.success(),
        "stderr:\n{}\nloopback requests: {}",
        stderr(&output),
        requests.len()
    );
    let out = stdout(&output);
    assert!(
        out.contains("loopback inspection complete"),
        "final model response was not rendered:\n{out}"
    );
    assert_eq!(requests.len(), 2, "expected one tool continuation");
    let second = String::from_utf8_lossy(&requests[1]);
    assert!(second.contains("function_call_output"));
    assert!(second.contains("provider-call-read"));
    assert!(second.contains("LOOPBACK_VALUE"));
    assert!(!out.contains("loopback-provider-secret-canary"));
    let _ = std::fs::remove_dir_all(root);
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
