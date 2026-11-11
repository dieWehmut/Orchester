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
    desktop_services: BTreeMap<String, String>,
    qwen_value_flags: Vec<String>,
    qwen_maintenance_commands: Vec<String>,
    openclaw_root_value_flags: Vec<String>,
    openclaw_root_switch_flags: Vec<String>,
    openclaw_gateway_value_flags: Vec<String>,
    openclaw_gateway_switch_flags: Vec<String>,
    native_cli: BTreeMap<String, String>,
    python_modules: BTreeMap<String, String>,
    cli_maintenance_commands: BTreeMap<String, Vec<String>>,
    desktop_brands: BTreeMap<String, String>,
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
        || rules().desktop_services.contains_key(&name)
        || rules().native_cli.contains_key(&name)
        || rules().runtimes.contains(&name)
        || rules().candidates.contains(&name)
        || name.starts_with("python3.")
        || name.starts_with("pypy3.")
}
fn suffix(value: &str, suffix: &str) -> bool {
    value == suffix || value.ends_with(&format!("/{suffix}"))
}
fn cursor_cli_file(value: &str) -> Option<&str> {
    let (_, versioned) = value.rsplit_once("/cursor-agent/versions/")?;
    let (version, file) = versioned.split_once('/')?;
    (!version.is_empty() && !file.contains('/')).then_some(file)
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
    let executable = basename(&exe);
    let desktop = desktop_provider(&exe);
    // The desktop and native CLI both use Claude.exe. Resolve the actual
    // executable before considering the primary CLI name.
    if name == "claude" && exe.is_empty() {
        return None;
    }
    if let Some(provider) = desktop {
        return desktop_main(args).then(|| provider.to_owned());
    }
    if let Some(provider) = rules().primary.get(&name) {
        return Some(provider.clone());
    }
    if let Some(provider) = rules().native_cli.get(&executable) {
        return (!args.is_empty() && !cli_maintenance(provider, &args[1..]))
            .then(|| provider.clone());
    }
    if cursor_cli_file(&exe).is_some_and(|file| {
        matches!(
            file,
            "cursor-agent" | "cursor-agent.exe" | "agent" | "agent.exe"
        )
    }) {
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
    let python = executable.starts_with("python") || executable.starts_with("pypy");
    if python && name == "hermes" && args.first().is_some_and(|arg| basename(arg) == "hermes") {
        return Some("hermes".to_owned());
    }
    if python {
        let mut module_index = 1;
        while args
            .get(module_index)
            .is_some_and(|arg| matches!(arg.as_str(), "-u" | "-I" | "-B" | "-E" | "-s" | "-S"))
        {
            module_index += 1;
        }
        if args.get(module_index).is_some_and(|arg| arg == "-m") {
            let provider = rules().python_modules.get(args.get(module_index + 1)?)?;
            return (!cli_maintenance(provider, &args[module_index + 2..]))
                .then(|| provider.clone());
        }
    }
    // OpenClaw sets this Gateway title on Unix, replacing the original argv.
    // /proc/comm truncates it to 15 bytes; only the exact argv title and an
    // actual Node/Bun runtime establish this service identity.
    if args
        .first()
        .is_some_and(|arg| basename(arg) == "openclaw-gateway")
    {
        return Some("openclaw".to_owned());
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
    // Official launchers exec the bundled Node runtime with this script;
    // Linux preserves the invoked alias in argv[0] and may use system CAs.
    if cursor_cli_file(&entry) == Some("index.js") {
        return Some("cursor".to_owned());
    }
    let provider = rules()
        .entries
        .iter()
        .find_map(|(expected, provider)| suffix(&entry, expected).then_some(provider))?;
    if provider == "deepseek" && dsh_maintenance(&args[index + 1..]) {
        return None;
    }
    if provider == "qwen" && qwen_maintenance(&args[index + 1..]) {
        return None;
    }
    if provider == "openclaw" && !openclaw_gateway(&args[index + 1..]) {
        return None;
    }
    if rules().cli_maintenance_commands.contains_key(provider)
        && cli_maintenance(provider, &args[index + 1..])
    {
        return None;
    }
    Some(provider.clone())
}

fn desktop_provider(exe: &str) -> Option<&str> {
    if let Some(provider) = rules().desktop_services.get(&basename(exe)) {
        return Some(provider);
    }
    if let Some(provider) = rules()
        .desktop_brands
        .iter()
        .find_map(|(brand, provider)| suffix(exe, brand).then_some(provider))
    {
        return Some(provider);
    }
    if basename(exe) == "claude"
        && (exe.contains("/anthropicclaude/")
            || exe.split('/').any(|component| {
                component.starts_with("claude_") && component.ends_with("__pzs8sxrjxfjjc")
            }))
    {
        return Some("claude_app");
    }
    None
}

fn desktop_main(args: &[String]) -> bool {
    !args.is_empty()
        && !args.iter().any(|arg| {
            matches!(
                arg.split('=').next(),
                Some("--type" | "--crashpad-handler" | "--ms-enable-electron-run-as-node")
            )
        })
        && entry_index(args).map_or(true, |index| {
            ![".js", ".mjs", ".cjs", ".ts"]
                .iter()
                .any(|ext| path(&args[index]).ends_with(ext))
        })
}

fn cli_maintenance(provider: &str, args: &[String]) -> bool {
    let commands = rules().cli_maintenance_commands.get(provider);
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        let key = arg.split('=').next().unwrap_or_default();
        if matches!(
            key,
            "--prompt" | "-p" | "--message" | "--query" | "-q" | "--"
        ) {
            return false;
        }
        if rules().qwen_value_flags.iter().any(|flag| flag == key) {
            index += if arg.contains('=') { 1 } else { 2 };
            continue;
        }
        if rules().maintenance.contains(arg) || matches!(arg.as_str(), "-v" | "version") {
            return true;
        }
        if !arg.starts_with('-') {
            return commands.is_some_and(|commands| commands.contains(arg));
        }
        index += 1;
    }
    false
}

fn qwen_maintenance(args: &[String]) -> bool {
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        let key = arg.split('=').next().unwrap_or_default();
        if matches!(key, "--prompt" | "-p" | "--") {
            return false;
        }
        if rules().qwen_value_flags.iter().any(|flag| flag == key) {
            index += if arg.contains('=') { 1 } else { 2 };
            continue;
        }
        if rules().maintenance.contains(arg) || arg == "-v" {
            return true;
        }
        if !arg.starts_with('-') {
            return rules().qwen_maintenance_commands.contains(arg);
        }
        index += 1;
    }
    false
}

fn openclaw_gateway(args: &[String]) -> bool {
    let mut index = 0;
    let mut command = false;
    let mut run = false;
    while let Some(arg) = args.get(index) {
        let key = arg.split('=').next().unwrap_or_default();
        let value_flag = rules()
            .openclaw_root_value_flags
            .iter()
            .any(|flag| flag == key)
            || (command
                && rules()
                    .openclaw_gateway_value_flags
                    .iter()
                    .any(|flag| flag == key));
        if value_flag {
            if !arg.contains('=') {
                if !args
                    .get(index + 1)
                    .is_some_and(|value| !value.is_empty() && !value.starts_with('-'))
                {
                    return false;
                }
                index += 1;
            } else if arg.ends_with('=') {
                return false;
            }
        } else if rules()
            .openclaw_root_switch_flags
            .iter()
            .any(|flag| flag == key)
            || (command
                && rules()
                    .openclaw_gateway_switch_flags
                    .iter()
                    .any(|flag| flag == key))
        {
            if arg.contains('=') {
                return false;
            }
        } else if !command && arg == "gateway" {
            command = true;
        } else if command && !run && arg == "run" {
            run = true;
        } else {
            return false;
        }
        index += 1;
    }
    command
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
    if args.first().is_some_and(|arg| {
        let name = basename(arg);
        rules().native_cli.contains_key(&name)
            || rules().desktop_services.contains_key(&name)
            || rules().primary.contains_key(&name)
    }) {
        return false;
    }
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
    if desktop_provider(&path(exe)).is_some() {
        return format!("desktop:{}", path(exe));
    }
    if rules().primary.contains_key(&basename(name)) {
        return format!("native:{}", basename(name));
    }
    if let Some(provider) = rules().native_cli.get(&basename(exe)) {
        return cli_instance_key(provider, args.get(1..).unwrap_or_default(), true);
    }
    if args
        .first()
        .is_some_and(|arg| basename(arg) == "openclaw-gateway")
    {
        return "openclaw:gateway-title".to_owned();
    }
    if let Some(mut index) = entry_index(args) {
        if suffix(
            &path(&args[index]),
            "dsh desktop/resources/harness-node-entry.mjs",
        ) {
            index += 1;
        }
        if let Some(entry) = args.get(index) {
            if let Some(provider) = rules()
                .entries
                .iter()
                .find_map(|(expected, provider)| suffix(&path(entry), expected).then_some(provider))
            {
                if matches!(provider.as_str(), "cline" | "kilo" | "crush" | "hermes") {
                    return cli_instance_key(provider, &args[index + 1..], false);
                }
            }
            use sha2::{Digest, Sha256};
            let mut digest = Sha256::new();
            for arg in &args[index + 1..] {
                digest.update(arg.as_bytes());
                digest.update([0]);
            }
            return format!("{}:{:x}", canonical_entry(&path(entry)), digest.finalize());
        }
    }
    path(exe)
}

fn cli_instance_key(provider: &str, args: &[String], native: bool) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for arg in args {
        digest.update(arg.as_bytes());
        digest.update([0]);
    }
    format!(
        "{}:{provider}:{:x}",
        if native { "native" } else { "launcher" },
        digest.finalize()
    )
}

fn canonical_entry(entry: &str) -> String {
    // Official wrappers can respawn a different script in the same package.
    // Preserve the package root and CLI arguments to collapse only that pair.
    for (expected, provider) in &rules().entries {
        if matches!(provider.as_str(), "qwen" | "openclaw") && suffix(entry, expected) {
            let package = if expected.starts_with("@qwen-code/") {
                "@qwen-code/qwen-code"
            } else if provider == "qwen" {
                "qwen-code"
            } else {
                "openclaw"
            };
            return format!("{}{package}", entry.strip_suffix(expected).unwrap());
        }
    }
    entry.to_owned()
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
                        || native_launcher_pair(provider, parent_key, key)
                        || (provider == "openclaw" && key == "openclaw:gateway-title"))
            })
        }) {
            continue;
        }
        *counts.entry(provider.clone()).or_default() += 1;
    }
    counts
}

fn native_launcher_pair(provider: &str, parent_key: &str, key: &str) -> bool {
    if rules()
        .native_cli
        .values()
        .any(|candidate| candidate == provider)
    {
        if let Some(digest) = parent_key.strip_prefix(&format!("launcher:{provider}:")) {
            return key == format!("native:{provider}:{digest}");
        }
        // Python's Windows console entry is itself a small native launcher.
        if provider == "hermes" {
            if let Some(digest) = parent_key.strip_prefix("native:hermes:") {
                return key == format!("launcher:hermes:{digest}");
            }
        }
    }
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
    fn agentrouter_desktops_are_services_and_claude_cli_remains_separate() {
        let fixtures = serde_json::json!([
            {"name":"ZCode.exe", "exe":"C:/Apps/ZCode/ZCode.exe", "args":["ZCode.exe"], "provider":"zcode"},
            {"name":"ZCode.exe", "exe":"C:/Apps/ZCode/ZCode.exe", "args":["ZCode.exe", "--type=renderer"], "provider":null},
            {"name":"Claude.exe", "exe":"C:/Users/test/AppData/Local/AnthropicClaude/app-3.0/Claude.exe", "args":["Claude.exe"], "provider":"claude_app"},
            {"name":"Claude.exe", "exe":"C:/Program Files/WindowsApps/Claude_3.6556.2.0_x64__pzs8sxrjxfjjc/app/Claude.exe", "args":["Claude.exe"], "provider":"claude_app"},
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
}
