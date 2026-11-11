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
                (component.starts_with("claude_") && component.ends_with("__pzs8sxrjxfjjc"))
                    || (component.starts_with("anthropicpbc.claude_")
                        && component.ends_with("__fnn82j28hfe8t"))
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
#[path = "tests/identity.rs"]
mod tests;
