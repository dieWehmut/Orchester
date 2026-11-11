use super::*;
fn check(name: &str, exe: &str, args: &[&str], provider: Option<&str>) {
    let args = args
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    assert_eq!(classify(name, exe, &args).as_deref(), provider);
}

#[cfg(unix)]
fn wsl_fixture(fixtures: &serde_json::Value) -> super::super::IdentityCounts {
    use std::io::Write;
    let mut child = std::process::Command::new("python3")
        .args([
            "-c",
            include_str!("../wsl_query.py"),
            RULES_JSON,
            "--fixture",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(fixtures.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let result = super::super::parse_identity_counts(&output.stdout).unwrap();
    let wire = String::from_utf8(output.stdout).unwrap();
    for forbidden in [
        "pid", "exe", "args", "cwd", "env", "/opt", "prompt", "fixture",
    ] {
        assert!(!wire.contains(forbidden));
    }
    result
}

#[cfg(unix)]
#[test]
fn relative_dot_segments_do_not_turn_parent_entries_into_branded_services() {
    let mut fixtures = Vec::new();
    let mut expected = BTreeMap::new();
    for (index, (cwd, entry, provider)) in [
        (
            "/repo/deepseek-harness",
            "./apps/cli/src/bin.ts",
            Some("deepseek"),
        ),
        ("/repo/deepseek-harness", "../apps/cli/src/bin.ts", None),
        ("/repo/deepseek-harness", "./../apps/cli/src/bin.ts", None),
        (
            "/repo/work",
            "../deepseek-harness/apps/cli/src/bin.ts",
            Some("deepseek"),
        ),
        (
            r"C:\repo\deepseek-harness",
            r".\apps\cli\src\bin.ts",
            Some("deepseek"),
        ),
        (r"C:\repo\deepseek-harness", r"..\apps\cli\src\bin.ts", None),
        ("/opt/@google/gemini-cli", "./dist/index.js", Some("gemini")),
        ("/opt/@google/gemini-cli", "../dist/index.js", None),
        (
            "/repo",
            "/repo/deepseek-harness/../apps/cli/src/bin.ts",
            None,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut args = vec!["node".to_owned(), entry.to_owned()];
        resolve_relative_entry(&mut args, cwd);
        assert_eq!(
            classify("node", "/usr/bin/node", &args).as_deref(),
            provider
        );
        if let Some(provider) = provider {
            *expected.entry(provider.to_owned()).or_insert(0) += 1;
        }
        fixtures.push(serde_json::json!({
            "pid": index + 1, "name": "node", "exe": "/usr/bin/node",
            "cwd": cwd, "args": ["node", entry]
        }));
    }
    assert_eq!(
        path("../../apps/cli/src/bin.ts"),
        "../../apps/cli/src/bin.ts"
    );
    let result = wsl_fixture(&serde_json::Value::Array(fixtures));
    assert!(result.complete);
    assert_eq!(result.counts, expected);
}

#[cfg(unix)]
#[test]
fn partial_wsl_query_preserves_positives_but_skips_dead_and_truncated_candidates() {
    let mut too_many = vec![
        "node".to_owned(),
        "/opt/@google/gemini-cli/dist/index.js".to_owned(),
    ];
    too_many.extend(vec!["fixture".to_owned(); 64]);
    let fixtures = serde_json::json!([
        {"pid": 1, "name": "codex", "exe": "/usr/bin/codex", "args": []},
        {"pid": 2, "name": "python3", "unavailable": true},
        {"pid": 3, "name": "node", "exe": "/usr/bin/node", "args": too_many},
        {"pid": 4, "name": "node", "exe": "/usr/bin/node", "args": ["node", "/opt/@deepseek-ai/dsh/lib/bin.js", "fixture".repeat(MAX_METADATA_BYTES)]},
        {"pid": 5, "name": "claude", "exe": "/usr/bin/claude", "args": [], "state": "Z"},
        {"pid": 6, "name": "opencode", "exe": "/usr/bin/opencode", "args": [], "state": "X"},
        {"pid": 7, "name": "deepseek", "exe": "/usr/bin/deepseek", "args": [], "state": "x"},
        {"pid": 8, "name": "node", "exe": "/usr/bin/node", "args": ["node", "/opt/@google/gemini-cli/dist/index.js"]}
    ]);
    let result = wsl_fixture(&fixtures);
    assert!(!result.complete);
    assert_eq!(
        result.counts,
        BTreeMap::from([("codex".to_owned(), 1), ("gemini".to_owned(), 1)])
    );
}
#[test]
fn branded_agent_entries_are_recognized_across_runtime_wrappers() {
    for runtime in ["node", "bun", "pypy3"] {
        check(
            runtime,
            &format!("/usr/bin/{runtime}"),
            &[
                runtime,
                "/opt/node_modules/@google/gemini-cli/dist/index.js",
            ],
            Some("gemini"),
        );
        check(
            runtime,
            &format!("/usr/bin/{runtime}"),
            &[
                runtime,
                "/opt/node_modules/@deepseek-ai/dsh/lib/bin.js",
                "web",
            ],
            Some("deepseek"),
        );
    }
    check(
        "node.exe",
        r"C:\Program Files\nodejs\node.exe",
        &[
            "node",
            "--import",
            "tsx/esm",
            "/repo/deepseek-harness/apps/cli/src/bin.ts",
            "web",
        ],
        Some("deepseek"),
    );
    check("DSH Desktop.exe", r"C:\Program Files\DSH Desktop\DSH Desktop.exe", &["DSH Desktop.exe", "--expose-internals", "C:/Program Files/DSH Desktop/resources/harness-node-entry.mjs", "C:/Program Files/DSH Desktop/resources/app.asar/node_modules/@deepseek-ai/dsh/lib/bin.js", "web"], Some("deepseek"));
    check(
        "node.exe",
        r"C:\Program Files\nodejs\node.exe",
        &[
            "node",
            "C:/cursor/extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs",
        ],
        Some("cursor"),
    );
    check(
        "agent",
        "/home/test/.local/share/cursor-agent/versions/2026.11.05/cursor-agent",
        &["agent"],
        Some("cursor"),
    );
    check(
        "language_server",
        r"C:\Apps\Antigravity\resources\bin\language_server.exe",
        &[],
        Some("antigravity"),
    );
    check(
        "language_server",
        "/opt/other/language_server",
        &[
            "language_server",
            "--override_user_agent_name",
            "antigravity",
        ],
        Some("antigravity"),
    );
}

#[test]
fn current_gemini_bundle_uses_the_real_entrypoint_locally_and_in_wsl() {
    let entry = "/opt/node_modules/@google/gemini-cli/bundle/gemini.js";
    check("node", "/usr/bin/node", &["node", entry], Some("gemini"));
    check(
        "bun",
        "/usr/bin/bun",
        &["bun", "run", entry],
        Some("gemini"),
    );
    check(
        "node.exe",
        r"C:\Program Files\nodejs\node.exe",
        &[
            "node",
            "C:/npm/node_modules/@google/gemini-cli/bundle/gemini.js",
        ],
        Some("gemini"),
    );
    check("node", "/usr/bin/node", &["node", entry, "--version"], None);
    check(
        "node",
        "/usr/bin/node",
        &["node", entry, "mcp", "list"],
        None,
    );
    check(
        "node",
        "/usr/bin/node",
        &["node", entry, "--prompt", "mcp"],
        Some("gemini"),
    );
    check(
        "node",
        "/usr/bin/node",
        &["node", "/unrelated.js", entry],
        None,
    );
    #[cfg(unix)]
    {
        let rows = serde_json::json!([
            {"pid": 1, "name": "node", "exe": "/usr/bin/node", "args": ["node", entry]},
            {"pid": 2, "name": "node", "exe": "/usr/bin/node", "args": ["node", entry, "--version"]},
            {"pid": 3, "name": "node", "exe": "/usr/bin/node", "args": ["node", "/unrelated.js", entry]}
        ]);
        assert_eq!(
            wsl_fixture(&rows).counts,
            BTreeMap::from([("gemini".to_owned(), 1)])
        );
    }
}

#[test]
fn qwen_and_openclaw_entrypoints_agree_locally_and_in_wsl() {
    let fixtures = serde_json::json!([
        {"case":"published Qwen wrapper", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/node_modules/@qwen-code/qwen-code/cli-entry.js"], "provider":"qwen"},
        {"case":"published Qwen child", "name":"node.exe", "exe":"C:/node/node.exe", "args":["node", "--expose-gc", "C:/npm/node_modules/@qwen-code/qwen-code/cli.js", "--prompt", "private-fixture"], "provider":"qwen"},
        {"case":"older Qwen CLI", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/node_modules/@qwen-code/qwen-code/dist/index.js"], "provider":"qwen"},
        {"case":"Qwen source wrapper", "name":"node", "exe":"/usr/bin/node", "args":["node", "/repo/qwen-code/scripts/cli-entry.js"], "provider":"qwen"},
        {"case":"Qwen standalone Bun", "name":"bun", "exe":"/opt/qwen-code/bun/bin/bun", "args":["bun", "/opt/qwen-code/lib/cli-entry.js", "--acp"], "provider":"qwen"},
        {"case":"Qwen relative standalone child", "name":"node", "exe":"/usr/bin/node", "cwd":"/opt/qwen-code/lib", "args":["node", "./cli.js"], "provider":"qwen"},
        {"case":"Qwen version", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@qwen-code/qwen-code/cli-entry.js", "--version"], "provider":null},
        {"case":"Qwen management", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@qwen-code/qwen-code/cli.js", "mcp", "list"], "provider":null},
        {"case":"Qwen prompt is not a help invocation", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@qwen-code/qwen-code/cli.js", "--prompt", "--help"], "provider":"qwen"},
        {"case":"Qwen worker", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@qwen-code/qwen-code/execution-worker.js"], "provider":null},
        {"case":"unrelated Qwen project", "name":"node", "exe":"/usr/bin/node", "args":["node", "/repo/qwen/cli-entry.js"], "provider":null},
        {"case":"Qwen path in prompt", "name":"node", "exe":"/usr/bin/node", "args":["node", "/repo/app.js", "--prompt", "/opt/@qwen-code/qwen-code/cli.js"], "provider":null},
        {"case":"OpenClaw foreground gateway", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/node_modules/openclaw/openclaw.mjs", "gateway"], "provider":"openclaw"},
        {"case":"OpenClaw Windows gateway run", "name":"node.exe", "exe":"C:/node/node.exe", "args":["node", "C:/npm/node_modules/openclaw/dist/entry.js", "--profile", "work", "gateway", "run", "--port", "1234"], "provider":"openclaw"},
        {"case":"OpenClaw alternate compiled entry", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/dist/entry.mjs", "gateway", "--bind=loopback"], "provider":"openclaw"},
        {"case":"OpenClaw Unix gateway title", "name":"openclaw-gatewa", "exe":"/usr/bin/node", "args":["openclaw-gateway"], "provider":"openclaw"},
        {"case":"OpenClaw gateway status is not a gateway", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/openclaw.mjs", "gateway", "status"], "provider":null},
        {"case":"OpenClaw gateway start is management", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/openclaw.mjs", "gateway", "start"], "provider":null},
        {"case":"OpenClaw gateway help", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/openclaw.mjs", "gateway", "run", "--help"], "provider":null},
        {"case":"OpenClaw agent client", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/openclaw.mjs", "agent", "--message", "private-fixture"], "provider":null},
        {"case":"OpenClaw TUI client", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/openclaw.mjs", "tui"], "provider":null},
        {"case":"OpenClaw titled maintenance parent", "name":"openclaw", "exe":"/usr/bin/node", "args":["openclaw"], "provider":null},
        {"case":"OpenClaw gateway worker", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/dist/gateway-worker.js", "gateway"], "provider":null},
        {"case":"OpenClaw prompt path", "name":"node", "exe":"/usr/bin/node", "args":["node", "/repo/app.js", "/opt/openclaw/openclaw.mjs", "gateway"], "provider":null},
        {"case":"OpenClaw bare executable is unconfirmed", "name":"openclaw", "exe":"/usr/bin/openclaw", "args":["openclaw", "gateway"], "provider":null},
        {"case":"OpenClaw missing profile value", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/openclaw.mjs", "--profile", "--help", "gateway"], "provider":null}
    ]);
    for row in fixtures.as_array().unwrap() {
        let mut args = row["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        resolve_relative_entry(&mut args, row["cwd"].as_str().unwrap_or_default());
        let expected = row["provider"].as_str();
        assert_eq!(
            classify(
                row["name"].as_str().unwrap(),
                row["exe"].as_str().unwrap(),
                &args
            )
            .as_deref(),
            expected,
            "{}",
            row["case"]
        );
        if expected.is_some() {
            assert!(
                is_candidate(row["name"].as_str().unwrap()),
                "{}",
                row["case"]
            );
        }
        #[cfg(unix)]
        {
            let mut fixture = row.clone();
            fixture["pid"] = serde_json::json!(1);
            let result = wsl_fixture(&serde_json::json!([fixture]));
            assert!(result.complete, "{}", row["case"]);
            let counts = expected
                .map(|provider| BTreeMap::from([(provider.to_owned(), 1)]))
                .unwrap_or_default();
            assert_eq!(result.counts, counts, "{}", row["case"]);
        }
    }
}

#[test]
fn workbuddy_desktop_service_excludes_electron_and_node_helpers() {
    let fixtures = serde_json::json!([
        {"name":"WorkBuddy.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddy.exe", "args":["C:/Apps/WorkBuddy/WorkBuddy.exe"], "service":true},
        {"name":"MainThread", "exe":"/Applications/WorkBuddy.app/Contents/MacOS/WorkBuddy", "args":["/Applications/WorkBuddy.app/Contents/MacOS/WorkBuddy"], "service":true},
        {"name":"WorkBuddy.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddy.exe", "args":["WorkBuddy.exe", "--type=renderer"], "service":false},
        {"name":"WorkBuddy.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddy.exe", "args":["WorkBuddy.exe", "--type", "utility"], "service":false},
        {"name":"WorkBuddy.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddy.exe", "args":["WorkBuddy.exe", "--type=gpu-process"], "service":false},
        {"name":"WorkBuddy.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddy.exe", "args":["WorkBuddy.exe", "--expose-gc", "C:/Apps/WorkBuddy/resources/worker.js"], "service":false},
        {"name":"WorkBuddy.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddy.exe", "args":[], "service":false},
        {"name":"node", "exe":"/usr/bin/node", "args":["node", "/repo/app.js", "--prompt", "C:/Apps/WorkBuddy/WorkBuddy.exe"], "service":false}
    ]);
    for row in fixtures.as_array().unwrap() {
        let args = row["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let expected = row["service"].as_bool().unwrap().then_some("workbuddy");
        assert_eq!(
            classify(
                row["name"].as_str().unwrap(),
                row["exe"].as_str().unwrap(),
                &args
            )
            .as_deref(),
            expected
        );
        if expected.is_some() {
            assert!(is_candidate(row["name"].as_str().unwrap()));
        }
        #[cfg(unix)]
        {
            let mut fixture = row.clone();
            fixture["pid"] = serde_json::json!(1);
            let result = wsl_fixture(&serde_json::json!([fixture]));
            let counts = expected
                .map(|provider| BTreeMap::from([(provider.to_owned(), 1)]))
                .unwrap_or_default();
            assert_eq!(result.counts, counts);
        }
    }
}

#[test]
fn workbuddy_editions_remain_distinct_locally_and_in_wsl() {
    let fixtures = serde_json::json!([
        {"pid":1, "name":"WorkBuddy.exe", "exe":"C:/Apps/workbuddy/WorkBuddy.exe", "args":["C:/Apps/workbuddy/WorkBuddy.exe"], "provider":"workbuddy"},
        {"pid":2, "parent":1, "name":"WorkBuddyAI.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":["C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe"], "provider":"workbuddy_global"},
        {"pid":3, "name":"MainThread", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":["C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe"], "provider":"workbuddy_global"},
        {"pid":4, "parent":2, "name":"WorkBuddyAI.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":["WorkBuddyAI.exe", "--type=renderer"], "provider":null},
        {"pid":5, "parent":2, "name":"WorkBuddyAI.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":["WorkBuddyAI.exe", "--type", "utility"], "provider":null},
        {"pid":6, "parent":2, "name":"WorkBuddyAI.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":["WorkBuddyAI.exe", "--type=gpu-process"], "provider":null},
        {"pid":7, "parent":2, "name":"WorkBuddyAI.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":["WorkBuddyAI.exe", "--crashpad-handler"], "provider":null},
        {"pid":8, "parent":2, "name":"WorkBuddyAI.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":["WorkBuddyAI.exe", "--ms-enable-electron-run-as-node", "C:/Apps/WorkBuddy/resources/worker.js"], "provider":null},
        {"pid":9, "parent":2, "name":"WorkBuddyAI.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":["WorkBuddyAI.exe", "--expose-gc", "C:/Apps/WorkBuddy/resources/worker.js"], "provider":null},
        {"pid":10, "name":"WorkBuddyAI.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe", "args":[], "provider":null},
        {"pid":11, "name":"workbuddy-editor-sdk-supervisor.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddyAI/resources/workbuddy-editor-sdk-supervisor.exe", "args":["workbuddy-editor-sdk-supervisor.exe"], "provider":null},
        {"pid":12, "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@genie/workbuddy-desktop/main/index.js"], "provider":null},
        {"pid":13, "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/unrelated.js", "--prompt", "C:/Apps/WorkBuddy/WorkBuddyAI/WorkBuddyAI.exe"], "provider":null},
        {"pid":14, "name":"workbuddy_global.exe", "exe":"C:/Apps/WorkBuddy/workbuddy_global.exe", "args":["workbuddy_global.exe"], "provider":null}
    ]);
    let rows = fixtures
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| {
            let args = row["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            let name = row["name"].as_str().unwrap();
            let exe = row["exe"].as_str().unwrap();
            let expected = row["provider"].as_str();
            let provider = classify(name, exe, &args);
            assert_eq!(provider.as_deref(), expected, "fixture: {row}");
            if expected.is_some() {
                assert!(is_candidate(name));
                assert!(!relative_entry(&args));
            }
            #[cfg(unix)]
            {
                let result = wsl_fixture(&serde_json::json!([row]));
                assert!(result.complete);
                assert_eq!(
                    result.counts,
                    expected
                        .map(|provider| BTreeMap::from([(provider.to_owned(), 1)]))
                        .unwrap_or_default()
                );
            }
            provider.map(|provider| {
                (
                    row["pid"].as_u64().unwrap() as u32,
                    row["parent"].as_u64().map(|pid| pid as u32),
                    provider.clone(),
                    instance_key(name, exe, &args),
                )
            })
        })
        .collect::<Vec<_>>();
    let expected = BTreeMap::from([
        ("workbuddy".to_owned(), 1),
        ("workbuddy_global".to_owned(), 2),
    ]);
    assert_eq!(count_instances(&rows), expected);
    #[cfg(unix)]
    {
        let result = wsl_fixture(&fixtures);
        assert!(result.complete);
        assert_eq!(result.counts, expected);
    }
}

#[test]
fn agentrouter_cli_entrypoints_agree_locally_and_in_wsl() {
    let fixtures = serde_json::json!([
        {"name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@mariozechner/pi-coding-agent/dist/cli.js"], "provider":"pi"},
        {"name":"Craft Agents.exe", "exe":"C:/Apps/Craft Agents/Craft Agents.exe", "args":["Craft Agents.exe"], "provider":"craft"},
        {"name":"Craft Agents.exe", "exe":"C:/Apps/Craft Agents/Craft Agents.exe", "args":["Craft Agents.exe", "--type=utility"], "provider":null},
        {"name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/node_modules/cline/bin/cline"], "provider":"cline"},
        {"name":"cline.exe", "exe":"C:/npm/@cline/cli-windows-x64/bin/cline.exe", "args":["cline.exe", "private-fixture"], "provider":"cline"},
        {"name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@kilocode/cli/bin/kilo"], "provider":"kilo"},
        {"name":"kilo", "exe":"/opt/@kilocode/cli-linux-x64/bin/kilo", "args":["kilo", "run", "private-fixture"], "provider":"kilo"},
        {"name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@charmland/crush/run-crush.js"], "provider":"crush"},
        {"name":"crush.exe", "exe":"C:/Tools/crush.exe", "args":["crush.exe"], "provider":"crush"},
        {"name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@earendil-works/pi-coding-agent/dist/bundle/cli.js"], "provider":"pi"},
        {"name":"python3", "exe":"/usr/bin/python3", "args":["python3", "-m", "hermes_cli.main", "chat"], "provider":"hermes"},
        {"name":"python3", "exe":"/usr/bin/python3", "args":["python3", "-u", "-m", "hermes_cli", "chat"], "provider":"hermes"},
        {"name":"python.exe", "exe":"C:/Python/python.exe", "args":["python.exe", "C:/Tools/hermes-agent/.venv/Scripts/hermes.exe", "chat"], "provider":"hermes"},
        {"name":"hermes", "exe":"/usr/bin/python3", "args":["hermes"], "provider":"hermes"},
        {"name":"hermes.exe", "exe":"C:/Tools/hermes-agent/.venv/Scripts/hermes.exe", "args":["hermes.exe", "chat"], "provider":"hermes"},
        {"name":"cline", "exe":"/usr/bin/cline", "args":["cline", "--help"], "provider":null},
        {"name":"kilo", "exe":"/usr/bin/kilo", "args":["kilo", "auth", "login"], "provider":null},
        {"name":"crush", "exe":"/usr/bin/crush", "args":["crush", "--version"], "provider":null},
        {"name":"python3", "exe":"/usr/bin/python3", "args":["python3", "-m", "hermes_cli.main", "status"], "provider":null},
        {"name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@earendil-works/pi-coding-agent/dist/bundle/extension-worker.js"], "provider":null},
        {"name":"node", "exe":"/usr/bin/node", "args":["node", "/repo/app.js", "--prompt", "/opt/cline/bin/cline"], "provider":null},
        {"name":"python3", "exe":"/usr/bin/python3", "args":["python3", "-c", "import hermes_cli.main"], "provider":null},
        {"name":"Code.exe", "exe":"C:/Apps/VSCode/Code.exe", "args":["Code.exe", "--type=utility", "--extensionDevelopmentPath", "/opt/cline"], "provider":null}
    ]);
    for row in fixtures.as_array().unwrap() {
        let args = row["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let expected = row["provider"].as_str();
        assert_eq!(
            classify(
                row["name"].as_str().unwrap(),
                row["exe"].as_str().unwrap(),
                &args
            )
            .as_deref(),
            expected
        );
        if expected.is_some() {
            assert!(is_candidate(row["name"].as_str().unwrap()));
        }
        #[cfg(unix)]
        {
            let mut fixture = row.clone();
            fixture["pid"] = serde_json::json!(1);
            let result = wsl_fixture(&serde_json::json!([fixture]));
            assert!(result.complete);
            assert_eq!(
                result.counts,
                expected
                    .map(|provider| BTreeMap::from([(provider.to_owned(), 1)]))
                    .unwrap_or_default()
            );
        }
    }
}

#[test]
fn native_cli_arguments_are_not_resolved_as_relative_runtime_scripts() {
    for (name, exe, raw, expected) in [
        ("kilo", "/usr/bin/kilo", vec!["kilo", "auth", "login"], None),
        (
            "cline",
            "/usr/bin/cline",
            vec!["cline", "private-fixture"],
            Some("cline"),
        ),
        ("crush", "/usr/bin/crush", vec!["crush", "version"], None),
    ] {
        let mut args = raw.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        assert!(!relative_entry(&args));
        resolve_relative_entry(&mut args, "/repo");
        assert_eq!(args, raw);
        assert_eq!(classify(name, exe, &args).as_deref(), expected);
        #[cfg(unix)]
        {
            let result = wsl_fixture(
                &serde_json::json!([{"pid":1,"name":name,"exe":exe,"cwd":"/repo","args":raw}]),
            );
            assert_eq!(
                result.counts,
                expected
                    .map(|provider| BTreeMap::from([(provider.to_owned(), 1)]))
                    .unwrap_or_default()
            );
        }
    }
}

#[test]
fn agentrouter_native_launcher_pairs_keep_independent_native_children() {
    let fixtures = serde_json::json!([
        {"pid":1,"name":"node","exe":"/usr/bin/node","args":["node","/opt/cline/bin/cline","private-fixture"]},
        {"pid":2,"parent":1,"name":"cline","exe":"/opt/@cline/cli-linux-x64/bin/cline","args":["cline","private-fixture"]},
        {"pid":3,"name":"cline","exe":"/usr/bin/cline","args":["cline","private-fixture"]},
        {"pid":4,"parent":3,"name":"cline","exe":"/usr/bin/cline","args":["cline","private-fixture"]},
        {"pid":5,"name":"node","exe":"/usr/bin/node","args":["node","/opt/@kilocode/cli/bin/kilo","run","private-fixture"]},
        {"pid":6,"parent":5,"name":"kilo","exe":"/usr/bin/kilo","args":["kilo","run","private-fixture"]},
        {"pid":7,"name":"node","exe":"/usr/bin/node","args":["node","/opt/@charmland/crush/run-crush.js"]},
        {"pid":8,"parent":7,"name":"crush","exe":"/usr/bin/crush","args":["crush"]},
        {"pid":9,"parent":8,"name":"crush","exe":"/usr/bin/crush","args":["crush"]}
    ]);
    let rows = fixtures
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let args = row["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            let name = row["name"].as_str().unwrap();
            let exe = row["exe"].as_str().unwrap();
            (
                row["pid"].as_u64().unwrap() as u32,
                row["parent"].as_u64().map(|id| id as u32),
                classify(name, exe, &args).unwrap(),
                instance_key(name, exe, &args),
            )
        })
        .collect::<Vec<_>>();
    let expected = BTreeMap::from([
        ("cline".to_owned(), 3),
        ("kilo".to_owned(), 1),
        ("crush".to_owned(), 2),
    ]);
    assert_eq!(count_instances(&rows), expected);
    #[cfg(unix)]
    {
        let result = wsl_fixture(&fixtures);
        assert!(result.complete);
        assert_eq!(result.counts, expected);
    }
}

#[test]
fn installed_linux_desktop_entrypoints_agree_locally_and_in_wsl() {
    let fixtures = serde_json::json!([
        {"name":"@craft-agentelectron", "exe":"/opt/craft-agents/squashfs-root/@craft-agentelectron", "args":["/opt/craft-agents/squashfs-root/@craft-agentelectron"], "provider":"craft"},
        {"name":"@craft-agentele", "exe":"/opt/craft-agents/squashfs-root/@craft-agentelectron", "args":["/opt/craft-agents/squashfs-root/@craft-agentelectron"], "provider":"craft"},
        {"name":"MainThread", "exe":"/opt/craft-agents/squashfs-root/@craft-agentelectron", "args":["/opt/craft-agents/squashfs-root/@craft-agentelectron"], "provider":"craft"},
        {"name":"@craft-agentele", "exe":"/opt/craft-agents/squashfs-root/@craft-agentelectron", "args":["/opt/craft-agents/squashfs-root/@craft-agentelectron", "--type=renderer"], "provider":null},
        {"name":"@craft-agentele", "exe":"/opt/craft-agents/squashfs-root/@craft-agentelectron", "args":["/opt/craft-agents/squashfs-root/@craft-agentelectron", "/opt/craft-agents/squashfs-root/resources/app/dist/main.cjs"], "provider":null},
        {"name":"@craft-agentele", "exe":"/opt/unrelated/@craft-agentele", "args":["/opt/unrelated/@craft-agentele"], "provider":null},
        {"name":"craft-agents", "exe":"/opt/craft-agents/craft-agents", "args":["/opt/craft-agents/craft-agents"], "provider":null},
        {"name":"AppRun", "exe":"/opt/craft-agents/squashfs-root/AppRun", "args":["/opt/craft-agents/squashfs-root/AppRun"], "provider":null},
        {"name":"trae-cn", "exe":"/opt/trae/usr/share/trae-cn/trae-cn", "args":["/opt/trae/usr/share/trae-cn/trae-cn"], "provider":"trae"},
        {"name":"trae-cn", "exe":"/opt/trae/usr/share/trae-cn/trae-cn", "args":["/opt/trae/usr/share/trae-cn/trae-cn", "--type", "utility"], "provider":null},
        {"name":"trae-cn", "exe":"/opt/trae/usr/share/trae-cn/trae-cn", "args":["/opt/trae/usr/share/trae-cn/trae-cn", "--ms-enable-electron-run-as-node", "/opt/trae/usr/share/trae-cn/resources/app/out/cli.js"], "provider":null},
        {"name":"trae-cn-launcher", "exe":"/opt/trae/trae-cn-launcher", "args":["/opt/trae/trae-cn-launcher"], "provider":null},
        {"name":"workbuddy", "exe":"/opt/WorkBuddy/workbuddy", "args":["/opt/WorkBuddy/workbuddy"], "provider":"workbuddy"},
        {"name":"zcode", "exe":"/opt/ZCode/zcode", "args":["/opt/ZCode/zcode"], "provider":"zcode"}
    ]);
    for row in fixtures.as_array().unwrap() {
        let args = row["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let expected = row["provider"].as_str();
        assert_eq!(
            classify(
                row["name"].as_str().unwrap(),
                row["exe"].as_str().unwrap(),
                &args
            )
            .as_deref(),
            expected,
            "fixture: {row}"
        );
        if expected.is_some() {
            assert!(is_candidate(row["name"].as_str().unwrap()));
            assert!(!relative_entry(&args));
        }
        #[cfg(unix)]
        {
            let mut fixture = row.clone();
            fixture["pid"] = serde_json::json!(1);
            let result = wsl_fixture(&serde_json::json!([fixture]));
            assert!(result.complete);
            assert_eq!(
                result.counts,
                expected
                    .map(|provider| BTreeMap::from([(provider.to_owned(), 1)]))
                    .unwrap_or_default()
            );
        }
    }
}

#[test]
fn agentrouter_desktops_are_services_and_claude_cli_remains_separate() {
    let fixtures = serde_json::json!([
        {"name":"ZCode.exe", "exe":"C:/Apps/ZCode/ZCode.exe", "args":["ZCode.exe"], "provider":"zcode"},
        {"name":"ZCode.exe", "exe":"C:/Apps/ZCode/ZCode.exe", "args":["ZCode.exe", "--type=renderer"], "provider":null},
        {"name":"Claude.exe", "exe":"C:/Users/test/AppData/Local/AnthropicClaude/app-3.0/Claude.exe", "args":["Claude.exe"], "provider":"claude_app"},
        {"name":"Claude.exe", "exe":"C:/Program Files/WindowsApps/Claude_3.6556.2.0_x64__pzs8sxrjxfjjc/app/Claude.exe", "args":["Claude.exe"], "provider":"claude_app"},
        {"name":"Claude.exe", "exe":"C:/Program Files/WindowsApps/AnthropicPBC.Claude_1.0.0.0_x64__fnn82j28hfe8t/app/Claude.exe", "args":["Claude.exe"], "provider":"claude_app"},
        {"name":"MainThread", "exe":"C:/Program Files/WindowsApps/AnthropicPBC.Claude_1.0.0.0_arm64__fnn82j28hfe8t/app/Claude.exe", "args":["Claude.exe"], "provider":"claude_app"},
        {"name":"Claude.exe", "exe":"C:/Program Files/WindowsApps/AnthropicPBC.Claude_1.0.0.0_x64__fnn82j28hfe8t/app/Claude.exe", "args":["Claude.exe", "--type=renderer"], "provider":null},
        {"name":"Claude.exe", "exe":"C:/Program Files/WindowsApps/AnthropicPBC.Claude_1.0.0.0_x64__fnn82j28hfe8t/app/Claude.exe", "args":["Claude.exe", "--crashpad-handler"], "provider":null},
        {"name":"Claude.exe", "exe":"C:/Program Files/WindowsApps/AnthropicPBC.Claude_1.0.0.0_x64__fnn82j28hfe8t/app/Claude.exe", "args":["Claude.exe", "--ms-enable-electron-run-as-node", "C:/app/worker.js"], "provider":null},
        {"name":"ClaudeHelper.exe", "exe":"C:/Program Files/WindowsApps/AnthropicPBC.Claude_1.0.0.0_x64__fnn82j28hfe8t/app/ClaudeHelper.exe", "args":["ClaudeHelper.exe"], "provider":null},
        {"name":"Claude.exe", "exe":"C:/Program Files/WindowsApps/AnthropicPBC.ClaudeEvil_1.0.0.0_x64__fnn82j28hfe8t/app/Claude.exe", "args":["Claude.exe"], "provider":"claude"},
        {"name":"Claude.exe", "exe":"C:/Program Files/WindowsApps/AnthropicPBC.Claude_1.0.0.0_x64__unrelated/app/Claude.exe", "args":["Claude.exe"], "provider":"claude"},
        {"name":"node", "exe":"C:/Tools/node.exe", "args":["node.exe", "C:/unrelated.js", "--prompt", "C:/Program Files/WindowsApps/AnthropicPBC.Claude_1.0.0.0_x64__fnn82j28hfe8t/app/Claude.exe"], "provider":null},
        {"name":"MainThread", "exe":"/Applications/Claude.app/Contents/MacOS/Claude", "args":["/Applications/Claude.app/Contents/MacOS/Claude"], "provider":"claude_app"},
        {"name":"claude.exe", "exe":"C:/Tools/claude/claude.exe", "args":["claude.exe"], "provider":"claude"},
        {"name":"Claude.exe", "exe":"C:/Users/test/AppData/Local/AnthropicClaude/app-3.0/Claude.exe", "args":["Claude.exe", "--type=renderer"], "provider":null},
        {"name":"Trae.exe", "exe":"C:/Apps/Trae/Trae.exe", "args":["Trae.exe"], "provider":"trae"},
        {"name":"Trae CN.exe", "exe":"C:/Apps/Trae CN/Trae CN.exe", "args":["Trae CN.exe"], "provider":"trae"},
        {"name":"Trae.exe", "exe":"C:/Apps/Trae/Trae.exe", "args":["Trae.exe", "--ms-enable-electron-run-as-node", "C:/Apps/Trae/resources/app/out/cli.js"], "provider":null},
        {"name":"Trae.exe", "exe":"C:/Apps/Trae/Trae.exe", "args":["Trae.exe", "--type=utility"], "provider":null},
        {"name":"Craft Agents.exe", "exe":"C:/Apps/Craft Agents/Craft Agents.exe", "args":["Craft Agents.exe"], "provider":"craft"},
        {"name":"Craft Agents.exe", "exe":"C:/Apps/Craft Agents/Craft Agents.exe", "args":["Craft Agents.exe", "--type=gpu-process"], "provider":null}
    ]);
    for row in fixtures.as_array().unwrap() {
        let args = row["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        let expected = row["provider"].as_str();
        assert_eq!(
            classify(
                row["name"].as_str().unwrap(),
                row["exe"].as_str().unwrap(),
                &args
            )
            .as_deref(),
            expected
        );
        if expected.is_some() {
            assert!(is_candidate(row["name"].as_str().unwrap()));
        }
        #[cfg(unix)]
        {
            let mut fixture = row.clone();
            fixture["pid"] = serde_json::json!(1);
            let result = wsl_fixture(&serde_json::json!([fixture]));
            assert!(result.complete);
            assert_eq!(
                result.counts,
                expected
                    .map(|provider| BTreeMap::from([(provider.to_owned(), 1)]))
                    .unwrap_or_default()
            );
        }
    }
}

#[test]
fn qwen_wrappers_and_openclaw_gateway_titles_do_not_inflate_counts() {
    let fixtures = serde_json::json!([
        {"pid":1, "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@qwen-code/qwen-code/cli-entry.js", "--prompt", "one-fixture"]},
        {"pid":2, "parent":1, "name":"node", "exe":"/usr/bin/node", "args":["node", "--expose-gc", "/opt/@qwen-code/qwen-code/cli.js", "--prompt", "one-fixture"]},
        {"pid":3, "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/@qwen-code/qwen-code/cli.js", "--prompt", "two-fixture"]},
        {"pid":4, "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/openclaw/openclaw.mjs", "gateway", "run"]},
        {"pid":5, "parent":4, "name":"openclaw-gatewa", "exe":"/usr/bin/node", "args":["openclaw-gateway"]},
        {"pid":6, "name":"openclaw-gatewa", "exe":"/usr/bin/node", "args":["openclaw-gateway"]},
        {"pid":7, "name":"WorkBuddy.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddy.exe", "args":["WorkBuddy.exe"]},
        {"pid":8, "parent":7, "name":"WorkBuddy.exe", "exe":"C:/Apps/WorkBuddy/WorkBuddy.exe", "args":["WorkBuddy.exe", "--type=utility"]}
    ]);
    let rows = fixtures
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| {
            let args = row["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            let name = row["name"].as_str().unwrap();
            let exe = row["exe"].as_str().unwrap();
            let provider = classify(name, exe, &args)?;
            Some((
                row["pid"].as_u64().unwrap() as u32,
                row["parent"].as_u64().map(|id| id as u32),
                provider,
                instance_key(name, exe, &args),
            ))
        })
        .collect::<Vec<_>>();
    let expected = BTreeMap::from([
        ("qwen".to_owned(), 2),
        ("openclaw".to_owned(), 2),
        ("workbuddy".to_owned(), 1),
    ]);
    assert_eq!(count_instances(&rows), expected);
    #[cfg(unix)]
    {
        let result = wsl_fixture(&fixtures);
        assert!(result.complete);
        assert_eq!(result.counts, expected);
    }
}

#[test]
fn official_cursor_cli_entrypoints_agree_locally_and_in_wsl() {
    let version = "/home/test/.local/share/cursor-agent/versions/2026.11.05-86047df";
    let node = format!("{version}/node");
    let entry = format!("{version}/index.js");
    let fixtures = serde_json::json!([
        {"case":"linux exec-a cursor-agent with system CAs", "name":"cursor-agent", "exe":node, "args":["/home/test/.local/bin/cursor-agent", "--use-system-ca", entry, "--prompt", "cursor-sensitive-fixture"], "cursor":true},
        {"case":"linux exec-a agent without system CAs", "name":"agent", "exe":node, "args":["/home/test/.local/bin/agent", entry], "cursor":true},
        {"case":"linux runtime process name", "name":"node", "exe":node, "args":["/home/test/.local/bin/agent", "--use-system-ca", entry], "cursor":true},
        {"case":"windows official runtime and entry", "name":"node.exe", "exe":"C:\\Users\\test\\AppData\\Local\\cursor-agent\\versions\\2026.11.05-86047df\\node.exe", "args":["node.exe", "C:\\Users\\test\\AppData\\Local\\cursor-agent\\versions\\2026.11.05-86047df\\index.js"], "cursor":true},
        {"case":"versioned relative entry", "name":"node", "exe":node, "cwd":version, "args":["node", "--use-system-ca", "./index.js"], "cursor":true},
        {"case":"timestamped release", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/cursor-agent/versions/2026.11.05-12-34-56-86047df/index.js"], "cursor":true},
        {"case":"runtime separator", "name":"node", "exe":node, "args":["node", "--use-system-ca", "--", entry], "cursor":true},
        {"case":"existing versioned executable", "name":"agent", "exe":format!("{version}/cursor-agent"), "args":["agent"], "cursor":true},
        {"case":"alias basename alone", "name":"cursor-agent", "exe":"/usr/bin/cursor-agent", "args":["cursor-agent"], "cursor":false},
        {"case":"branded runtime without entry", "name":"node", "exe":node, "args":["node"], "cursor":false},
        {"case":"ordinary project directory", "name":"node", "exe":"/usr/bin/node", "args":["node", "/repo/cursor-agent/index.js"], "cursor":false},
        {"case":"unversioned relative entry", "name":"node", "exe":"/usr/bin/node", "cwd":"/repo/cursor-agent", "args":["node", "index.js"], "cursor":false},
        {"case":"missing version", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/cursor-agent/versions/index.js"], "cursor":false},
        {"case":"empty version", "name":"node", "exe":"/usr/bin/node", "args":["node", "/opt/cursor-agent/versions//index.js"], "cursor":false},
        {"case":"version traversal", "name":"node", "exe":"/usr/bin/node", "args":["node", format!("{version}/../index.js")], "cursor":false},
        {"case":"nested helper index", "name":"node", "exe":node, "args":["node", format!("{version}/workers/index.js")], "cursor":false},
        {"case":"named helper script", "name":"node", "exe":node, "args":["node", format!("{version}/connect-pool-worker.js")], "cursor":false},
        {"case":"helper binary", "name":"cursor-agent-worker-sea", "exe":format!("{version}/cursor-agent-worker-sea"), "args":["cursor-agent-worker-sea"], "cursor":false},
        {"case":"cursor desktop", "name":"Cursor.exe", "exe":"C:/Apps/Cursor/Cursor.exe", "args":["Cursor.exe", entry], "cursor":false},
        {"case":"shell interpreting script", "name":"bash", "exe":"/bin/bash", "args":["bash", entry], "cursor":false},
        {"case":"prompt contains entry", "name":"node", "exe":"/usr/bin/node", "args":["node", "/repo/index.js", "--prompt", entry], "cursor":false},
        {"case":"runtime import contains entry", "name":"node", "exe":"/usr/bin/node", "args":["node", "--import", entry, "/repo/index.js"], "cursor":false},
        {"case":"runtime eval contains entry", "name":"node", "exe":node, "args":["node", "--eval", entry], "cursor":false},
        {"case":"unknown runtime option", "name":"node", "exe":node, "args":["node", "--unknown-option", entry], "cursor":false}
    ]);
    for row in fixtures.as_array().unwrap() {
        let mut args = row["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        resolve_relative_entry(&mut args, row["cwd"].as_str().unwrap_or_default());
        let expected = row["cursor"].as_bool().unwrap().then_some("cursor");
        assert_eq!(
            classify(
                row["name"].as_str().unwrap(),
                row["exe"].as_str().unwrap(),
                &args,
            )
            .as_deref(),
            expected,
            "{}",
            row["case"],
        );
        if expected.is_some() {
            assert!(
                is_candidate(row["name"].as_str().unwrap()),
                "{}",
                row["case"]
            );
        }
        #[cfg(unix)]
        {
            let mut fixture = row.clone();
            fixture["pid"] = serde_json::json!(1);
            let result = wsl_fixture(&serde_json::json!([fixture]));
            assert!(result.complete, "{}", row["case"]);
            let counts = if expected.is_some() {
                BTreeMap::from([("cursor".to_owned(), 1)])
            } else {
                BTreeMap::new()
            };
            assert_eq!(result.counts, counts, "{}", row["case"]);
        }
    }
}

#[test]
fn helpers_generic_names_and_prompt_strings_are_never_agent_identities() {
    for (name, exe) in [
        ("Cursor.exe", "C:/cursor/Cursor.exe"),
        ("Antigravity.exe", "C:/Antigravity/Antigravity.exe"),
        ("agent", "/usr/bin/agent"),
        ("gemini", "/usr/bin/gemini"),
        ("dsh", "/usr/bin/dsh"),
        (
            "language_server",
            "/opt/windsurf/resources/bin/language_server.exe",
        ),
    ] {
        check(name, exe, &[name], None);
    }
    for entry in [
        "/opt/agent-worker.js",
        "/opt/agent-exec.js",
        "/opt/unrelated/dist/index.js",
        "/opt/cursor-agent-host/daemon.cjs",
    ] {
        check(
            "node",
            "/usr/bin/node",
            &[
                "node",
                entry,
                "--prompt",
                "/opt/@google/gemini-cli/dist/index.js",
            ],
            None,
        );
    }
    check(
        "node",
        "/usr/bin/node",
        &["node", "-e", "require('/opt/@deepseek-ai/dsh/lib/bin.js')"],
        None,
    );
    check(
        "node",
        "/usr/bin/node",
        &["node", "--eval=/opt/@google/gemini-cli/dist/index.js"],
        None,
    );
    check(
        "node",
        "/usr/bin/node",
        &["node", "/opt/@deepseek-ai/dsh/lib/bin.js", "plugin"],
        None,
    );
    check(
        "node",
        "/usr/bin/node",
        &["node", "/opt/@deepseek-ai/dsh/lib/bin.js", "web", "--help"],
        None,
    );
    check(
        "node",
        "/usr/bin/node",
        &[
            "node",
            "/opt/@deepseek-ai/dsh/lib/bin.js",
            "web",
            "--prompt",
            "--help",
        ],
        Some("deepseek"),
    );
}
#[test]
fn supervisor_child_pairs_count_once_and_independent_instances_remain_separate() {
    let rows = vec![
        (1, None, "gemini".to_owned(), "same".to_owned()),
        (2, Some(1), "gemini".to_owned(), "same".to_owned()),
        (3, None, "gemini".to_owned(), "other".to_owned()),
        (4, Some(3), "gemini".to_owned(), "other".to_owned()),
        (5, None, "cursor".to_owned(), "host".to_owned()),
        (6, Some(5), "deepseek".to_owned(), "web".to_owned()),
    ];
    assert_eq!(
        count_instances(&rows),
        BTreeMap::from([
            ("gemini".to_owned(), 2),
            ("cursor".to_owned(), 1),
            ("deepseek".to_owned(), 1)
        ])
    );
}
#[test]
fn metadata_prefix_has_fixed_argument_and_byte_limits() {
    let many = vec![std::ffi::OsString::from("x"); 100];
    assert_eq!(bounded_args(&many).0.len(), 64);
    assert!(!bounded_args(&many).1);
    assert!(
        bounded_args(&[std::ffi::OsString::from("x".repeat(16 * 1024))])
            .0
            .is_empty()
    );
}
#[test]
fn relative_source_entries_flags_and_maintenance_follow_the_real_entrypoint() {
    check(
        "bun",
        "/usr/bin/bun",
        &["bun", "run", "/opt/@google/gemini-cli/dist/index.js"],
        Some("gemini"),
    );
    check(
        "language_server",
        "/opt/other/language_server",
        &["language_server", "--override_user_agent_name=Antigravity"],
        Some("antigravity"),
    );
    let mut args = vec!["node", "--import", "tsx/esm", "apps/cli/src/bin.ts", "web"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    resolve_relative_entry(&mut args, "/repo/deepseek-harness");
    assert_eq!(
        classify("node", "/usr/bin/node", &args).as_deref(),
        Some("deepseek")
    );
    check(
        "node",
        "/usr/bin/node",
        &[
            "node",
            "--max-old-space-size",
            "4096",
            "/opt/@google/gemini-cli/dist/index.js",
        ],
        Some("gemini"),
    );
    check(
        "language_server",
        "/opt/antigravity/resources/bin/language_server",
        &[],
        Some("antigravity"),
    );
    check(
        "node",
        "/usr/bin/node",
        &[
            "node",
            "/opt/@deepseek-ai/dsh/lib/bin.js",
            "--profile",
            "plugin",
            "headless",
            "help",
        ],
        Some("deepseek"),
    );
    check(
        "node",
        "/usr/bin/node",
        &[
            "node",
            "/opt/@deepseek-ai/dsh/lib/bin.js",
            "--dump-config-schema",
        ],
        None,
    );
    check(
        "node",
        "/usr/bin/node",
        &[
            "node",
            "/opt/@deepseek-ai/dsh/lib/bin.js",
            "--profile",
            "headless",
            "help",
        ],
        Some("deepseek"),
    );
    check(
        "node",
        "/usr/bin/node",
        &[
            "node",
            "/opt/@deepseek-ai/dsh/lib/bin.js",
            "--profile",
            "headless",
            "plugin",
        ],
        Some("deepseek"),
    );
    check(
        "node",
        "/usr/bin/node",
        &[
            "node",
            "/opt/@deepseek-ai/dsh/lib/bin.js",
            "--dump-default-config",
        ],
        None,
    );
    check(
        "node",
        "/usr/bin/node",
        &["node", "/opt/@deepseek-ai/dsh/lib/bin.js", "-V"],
        None,
    );
    check(
        "node",
        "/usr/bin/node",
        &["node", "/opt/@deepseek-ai/dsh/lib/bin.js", "web", "-v"],
        Some("deepseek"),
    );
    let native = vec![
        (10, None, "codex".to_owned(), "native:codex".to_owned()),
        (11, Some(10), "codex".to_owned(), "native:codex".to_owned()),
        (
            12,
            None,
            "codex".to_owned(),
            "/opt/@openai/codex/bin/codex.js:args".to_owned(),
        ),
        (13, Some(12), "codex".to_owned(), "native:codex".to_owned()),
    ];
    assert_eq!(count_instances(&native).get("codex"), Some(&3));
    let rows = vec![
        (1, None, "gemini".to_owned(), "entry-a:args-a".to_owned()),
        (2, Some(1), "gemini".to_owned(), "entry-b:args-a".to_owned()),
        (3, Some(2), "gemini".to_owned(), "entry-b:args-b".to_owned()),
    ];
    assert_eq!(count_instances(&rows).get("gemini"), Some(&3));
}
#[cfg(unix)]
#[test]
fn wsl_and_local_rules_agree_and_wsl_stdout_contains_only_safe_counts() {
    let fixtures = serde_json::json!([
        {"pid":1,"parent":null,"name":"node","exe":"/usr/bin/node","args":["node","/opt/@google/gemini-cli/dist/index.js"]},
        {"pid":2,"parent":1,"name":"node","exe":"/usr/bin/node","args":["node","/opt/@google/gemini-cli/dist/index.js"]},
        {"pid":3,"parent":null,"name":"bun","exe":"/usr/bin/bun","args":["bun","/opt/@deepseek-ai/dsh/lib/bin.js","web"]},
        {"pid":4,"parent":null,"name":"node","exe":"/usr/bin/node","args":["node","/cursor/extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs"]},
        {"pid":5,"parent":null,"name":"language_server","exe":"/opt/windsurf/language_server","args":["language_server","--override_user_agent_name=antigravity"]},
        {"pid":6,"parent":null,"name":"node","exe":"/usr/bin/node","args":["node","/unrelated.js","--prompt","/opt/@google/gemini-cli/dist/index.js"]},
        {"pid":8,"parent":null,"name":"node","exe":"/usr/bin/node","args":["node","/opt/@deepseek-ai/dsh/lib/bin.js","--profile","headless","help"]},
        {"pid":9,"parent":null,"name":"node","exe":"/usr/bin/node","cwd":"/repo/deepseek-harness","args":["node","--import","tsx/esm","apps/cli/src/bin.ts","web"]},
        {"pid":7,"parent":null,"name":"DSH Desktop.exe","exe":"C:/Program Files/DSH Desktop/DSH Desktop.exe","args":["DSH Desktop.exe","--expose-internals","C:/Program Files/DSH Desktop/resources/harness-node-entry.mjs","C:/Program Files/DSH Desktop/resources/app.asar/node_modules/@deepseek-ai/dsh/lib/bin.js","web"]}
    ]);
    let rows = fixtures
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| {
            let mut args = row["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            resolve_relative_entry(&mut args, row["cwd"].as_str().unwrap_or_default());
            classify(
                row["name"].as_str().unwrap(),
                row["exe"].as_str().unwrap(),
                &args,
            )
            .map(|provider| {
                (
                    row["pid"].as_u64().unwrap() as u32,
                    row["parent"].as_u64().map(|pid| pid as u32),
                    provider,
                    instance_key(
                        row["name"].as_str().unwrap(),
                        row["exe"].as_str().unwrap(),
                        &args,
                    ),
                )
            })
        })
        .collect::<Vec<_>>();
    let result = wsl_fixture(&fixtures);
    assert!(result.complete);
    let counts = result.counts;
    assert_eq!(counts, count_instances(&rows));
    assert_eq!(counts.get("gemini"), Some(&1));
    assert_eq!(counts.get("deepseek"), Some(&4));
}
