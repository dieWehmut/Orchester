//! Ephemeral identity classification. Only provider counts leave this module.
//! Rust and the WSL reader consume the same exact branding/entrypoint rules.
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) const RULES_JSON: &str = include_str!("rules.json");
const MAX_ARGS: usize = 64;
pub(super) const MAX_METADATA_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
struct Rules {
    primary: BTreeMap<String, String>,
    runtimes: Vec<String>,
    candidates: Vec<String>,
    entries: BTreeMap<String, String>,
    runtime_value_flags: Vec<String>,
    runtime_switch_flags: Vec<String>,
    maintenance: Vec<String>,
    dsh_value_flags: Vec<String>,
}
fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| serde_json::from_str(RULES_JSON).expect("static identity rules"))
}
fn path(value: &str) -> String {
    let value = value.replace('\\', "/").to_ascii_lowercase();
    let rooted = value.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." if parts
                .last()
                .is_some_and(|last| *last != ".." && !last.ends_with(':')) =>
            {
                parts.pop();
            }
            ".." if rooted || parts.last().is_some_and(|last| last.ends_with(':')) => {}
            _ => parts.push(part),
        }
    }
    format!("{}{}", if rooted { "/" } else { "" }, parts.join("/"))
}
fn basename(value: &str) -> String {
    let normalized = path(value);
    let name = normalized.rsplit('/').next().unwrap_or_default();
    name.strip_suffix(".exe").unwrap_or(name).to_owned()
}
pub(super) fn is_candidate(name: &str) -> bool {
    let name = basename(name);
    rules().primary.contains_key(&name)
        || rules().runtimes.contains(&name)
        || rules().candidates.contains(&name)
        || name.starts_with("python3.")
        || name.starts_with("pypy3.")
}
fn suffix(value: &str, suffix: &str) -> bool {
    value == suffix || value.ends_with(&format!("/{suffix}"))
}
fn cursor_cli(value: &str) -> bool {
    let Some((_, versioned)) = value.rsplit_once("/cursor-agent/versions/") else {
        return false;
    };
    let segments = versioned.split('/').collect::<Vec<_>>();
    segments.len() == 2
        && !segments[0].is_empty()
        && matches!(
            segments[1],
            "cursor-agent" | "cursor-agent.exe" | "agent" | "agent.exe"
        )
}
fn entry_index(args: &[String]) -> Option<usize> {
    let mut index = if args.first().is_some_and(|arg| basename(arg) == "bun")
        && args.get(1).is_some_and(|arg| arg == "run")
    {
        2
    } else {
        1
    };
    while let Some(arg) = args.get(index) {
        if matches!(arg.as_str(), "-e" | "--eval" | "-p" | "--print" | "-c") {
            return None;
        }
        if arg == "--" {
            return args.get(index + 1).map(|_| index + 1);
        }
        let key = arg.split('=').next().unwrap_or_default();
        if rules().runtime_value_flags.iter().any(|flag| flag == key) {
            index += if arg.contains('=') { 1 } else { 2 };
        } else if rules().runtime_switch_flags.iter().any(|flag| flag == key) {
            index += 1;
        } else if arg.starts_with('-') {
            return None;
        } else {
            return Some(index);
        }
    }
    None
}

/// `args` must be a bounded prefix. Trailing CLI prompts and keys are never
/// searched: only runtime flags, the real entrypoint and explicit brand flags.
pub(super) fn classify(name: &str, exe: &str, args: &[String]) -> Option<String> {
    let name = basename(name);
    let exe = path(exe);
    if let Some(provider) = rules().primary.get(&name) {
        return Some(provider.clone());
    }
    let executable = basename(&exe);
    if cursor_cli(&exe) {
        return Some("cursor".to_owned());
    }
    if name.starts_with("language_server") || executable.starts_with("language_server") {
        let branded = suffix(&exe, "antigravity/resources/bin/language_server")
            || suffix(&exe, "antigravity/resources/bin/language_server.exe")
            || suffix(&exe, "antigravity/resources/bin/language_server_linux_x64")
            || suffix(&exe, "antigravity/resources/bin/language_server_linux_arm");
        let explicit = args.iter().enumerate().any(|(index, arg)| {
            arg.strip_prefix("--override_user_agent_name=")
                .is_some_and(|value| value.eq_ignore_ascii_case("antigravity"))
                || (arg == "--override_user_agent_name"
                    && args
                        .get(index + 1)
                        .is_some_and(|value| value.eq_ignore_ascii_case("antigravity")))
        });
        return (branded || explicit).then(|| "antigravity".to_owned());
    }
    let runtime = rules().runtimes.contains(&executable)
        || executable.starts_with("python3.")
        || executable.starts_with("pypy3.")
        || (executable == "dsh desktop" && exe.contains("/dsh desktop/"));
    if !runtime {
        return None;
    }
    let mut index = entry_index(args)?;
    let mut entry = path(args.get(index)?);
    if suffix(&entry, "dsh desktop/resources/harness-node-entry.mjs") {
        index += 1;
        entry = path(args.get(index)?);
        if !suffix(&entry, "@deepseek-ai/dsh/lib/bin.js") {
            return None;
        }
    }
    let provider = rules()
        .entries
        .iter()
        .find_map(|(expected, provider)| suffix(&entry, expected).then_some(provider))?;
    if provider == "deepseek" && dsh_maintenance(&args[index + 1..]) {
        return None;
    }
    Some(provider.clone())
}

fn dsh_maintenance(args: &[String]) -> bool {
    let mut index = 0;
    if args.first().is_some_and(|arg| arg == "plugin") {
        return true;
    }
    while let Some(arg) = args.get(index) {
        if matches!(arg.as_str(), "--prompt" | "-p" | "--") {
            break;
        }
        let key = arg.split('=').next().unwrap_or_default();
        if rules().dsh_value_flags.iter().any(|flag| flag == key) {
            index += if arg.contains('=') { 1 } else { 2 };
            continue;
        }
        if key.starts_with("--dump-config")
            || key == "--dump-default-config"
            || (arg.starts_with('-') && rules().maintenance.contains(arg))
        {
            return true;
        }
        index += 1;
    }
    false
}

pub(super) fn relative_entry(args: &[String]) -> bool {
    entry_index(args)
        .and_then(|index| args.get(index))
        .is_some_and(|arg| {
            let entry = path(arg);
            !entry.starts_with('/') && entry.as_bytes().get(1) != Some(&b':')
        })
}
pub(super) fn resolve_relative_entry(args: &mut [String], cwd: &str) {
    if !cwd.is_empty() && relative_entry(args) {
        let index = entry_index(args).expect("relative entry exists");
        args[index] = format!("{}/{}", cwd.trim_end_matches(['/', '\\']), args[index]);
    }
}

pub(super) fn bounded_args<'a>(
    args: impl IntoIterator<Item = &'a std::ffi::OsString>,
) -> (Vec<String>, bool) {
    let mut bytes = 0;
    let mut result = Vec::new();
    for arg in args.into_iter().take(MAX_ARGS + 1) {
        let value = arg.to_string_lossy();
        bytes += value.len() + 1;
        if result.len() >= MAX_ARGS || bytes > MAX_METADATA_BYTES {
            return (result, false);
        }
        result.push(value.into_owned());
    }
    (result, true)
}

/// The canonical entry stays in memory. Matching entries prevent unrelated
/// services of the same provider from being collapsed as supervisors.
pub(super) fn instance_key(name: &str, exe: &str, args: &[String]) -> String {
    if rules().primary.contains_key(&basename(name)) {
        return format!("native:{}", basename(name));
    }
    if let Some(mut index) = entry_index(args) {
        if suffix(
            &path(&args[index]),
            "dsh desktop/resources/harness-node-entry.mjs",
        ) {
            index += 1;
        }
        if let Some(entry) = args.get(index) {
            use sha2::{Digest, Sha256};
            let mut digest = Sha256::new();
            for arg in &args[index + 1..] {
                digest.update(arg.as_bytes());
                digest.update([0]);
            }
            return format!("{}:{:x}", path(entry), digest.finalize());
        }
    }
    path(exe)
}

pub(super) fn count_instances(
    rows: &[(u32, Option<u32>, String, String)],
) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for (_, parent, provider, key) in rows {
        if parent.is_some_and(|parent| {
            rows.iter().any(|(pid, _, candidate, parent_key)| {
                *pid == parent
                    && candidate == provider
                    && ((parent_key == key && !key.starts_with("native:"))
                        || native_launcher_pair(provider, parent_key, key))
            })
        }) {
            continue;
        }
        *counts.entry(provider.clone()).or_default() += 1;
    }
    counts
}

fn native_launcher_pair(provider: &str, parent_key: &str, key: &str) -> bool {
    let launcher = match provider {
        "codex" => "/@openai/codex/bin/codex.js:",
        "claude" => "/@anthropic-ai/claude-code/cli.js:",
        "opencode" => "/opencode-ai/bin/opencode:",
        _ => return false,
    };
    key == format!("native:{provider}") && parent_key.contains(launcher)
}

#[cfg(test)]
mod tests {
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
            .args(["-c", include_str!("wsl_query.py"), RULES_JSON, "--fixture"])
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
}
