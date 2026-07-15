//! `orchester` — the CLI entry point.
//!
//! Flow: parse args → discover adapters into a [`Registry`] → either print the
//! adapter list, or build a [`Task`] and drive it through the [`Conductor`],
//! rendering each event (or emitting Event JSONL under `--json`). The process
//! exit code reflects the run outcome so scripts can branch on success/failure.

mod args;
mod avatar;
mod interactive;
mod render;

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
#[cfg(windows)]
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::process::{ExitCode, ExitStatus};

use clap::Parser;

use orchester_laufzeit::{Conductor, ConductorError, SessionRecord, SessionStore};
use orchester_protokoll::{Outcome, RunResult, Task};
use orchester_verzeichnis::{PluginRootError, Registry, standard_plugin_roots};

use args::{Cli, Command};
use interactive::{AgentChoice, PromptAction};

/// Directory holding on-disk manifests, relative to the current working dir.
const MANIFEST_DIR: &str = "manifeste";

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("orchester: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode, CliError> {
    let Cli {
        command,
        agent,
        resume,
        model,
        json,
        agents,
        parallel,
        auto,
        prompt,
    } = cli;

    let no_arg_launch = command.is_none()
        && agent.is_none()
        && resume.is_none()
        && model.is_none()
        && !json
        && agents.is_none()
        && !parallel
        && !auto
        && prompt.is_none();

    let registry = discover_registry()?;

    if no_arg_launch {
        return run_interactive(registry).await;
    }

    let prompt = match command {
        Some(Command::List) => {
            let mut out = io::stdout().lock();
            let caps = registry.list();
            if json {
                render::render_list_json(&mut out, &caps)?;
            } else {
                render::render_list(&mut out, &caps)?;
            }
            return Ok(ExitCode::SUCCESS);
        }
        Some(Command::Doctor(doctor)) => {
            let checks = registry.availability();
            let strict_failed = doctor.strict && checks.iter().any(|check| check.is_missing());
            let mut out = io::stdout().lock();
            render::render_doctor(&mut out, &checks)?;
            return Ok(if strict_failed {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            });
        }
        Some(Command::Sessions) => {
            let records = session_store().load()?;
            let mut out = io::stdout().lock();
            if json {
                render::render_sessions_json(&mut out, &records)?;
            } else {
                render::render_sessions(&mut out, &records)?;
            }
            return Ok(ExitCode::SUCCESS);
        }
        Some(Command::Run(run)) => run.prompt,
        None => prompt,
    };

    // Reserved multi-agent flags: declared to lock the UX, not yet implemented.
    if agents.is_some() || parallel || auto {
        eprintln!("orchester: multi-agent / --auto modes are not yet implemented (roadmap v0.5+)");
        return Ok(ExitCode::FAILURE);
    }

    // Default mode: run one agent.
    let agent = agent.ok_or(CliError::MissingAgent)?;
    let prompt = read_prompt(prompt)?;

    let conductor = Conductor::new(registry);
    let (record_task, result) =
        drive_agent_run(&conductor, &agent, prompt, resume, model, json).await?;

    if let Err(e) = record_session(&agent, &record_task, &result) {
        eprintln!("orchester: failed to record session metadata: {e}");
    }

    // In rendered mode, print a dim usage/outcome footer.
    if !json {
        let mut err = io::stderr().lock();
        let _ = writeln!(
            err,
            "\x1b[2m-> {:?} | tokens in {} / out {}\x1b[0m",
            result.outcome, result.usage.input_tokens, result.usage.output_tokens
        );
    }

    Ok(match result.outcome {
        Outcome::Success => ExitCode::SUCCESS,
        Outcome::Failed | Outcome::Cancelled => ExitCode::FAILURE,
    })
}

async fn run_interactive(registry: Registry) -> Result<ExitCode, CliError> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        return run_terminal_interactive(registry).await;
    }
    run_line_interactive(registry).await
}

async fn run_terminal_interactive(registry: Registry) -> Result<ExitCode, CliError> {
    loop {
        let choices = interactive::build_agent_choices(&registry);
        match interactive::run_home_tui(&choices)? {
            interactive::HomeAction::Quit => return Ok(ExitCode::SUCCESS),
            interactive::HomeAction::Help => {
                let mut out = io::stdout().lock();
                interactive::render_help(&mut out)?;
            }
            interactive::HomeAction::Submit(prompt) => {
                eprintln!(
                    "orchester: self-agent harness is not configured yet; received task `{}`. Use /agent or /codex to delegate.",
                    prompt
                );
            }
            interactive::HomeAction::Empty => {}
            interactive::HomeAction::PickAgent => {
                if let Some(agent) = interactive::select_agent_tui(&choices, None)? {
                    if agent.native_command.is_some() {
                        if launch_native_agent(&agent)? == NativeLaunchStatus::Cancelled {
                            return Ok(ExitCode::from(130));
                        }
                    } else {
                        run_adapter_prompt_shell(&registry, agent).await?;
                    }
                }
            }
            interactive::HomeAction::LaunchAgent(name) => {
                let Some(agent) = choices.iter().find(|choice| choice.name == name) else {
                    eprintln!("orchester: unknown or unavailable agent `{name}`");
                    continue;
                };
                if agent.native_command.is_some() {
                    if launch_native_agent(agent)? == NativeLaunchStatus::Cancelled {
                        return Ok(ExitCode::from(130));
                    }
                } else {
                    run_adapter_prompt_shell(&registry, agent.clone()).await?;
                }
            }
        }
    }
}

async fn run_line_interactive(registry: Registry) -> Result<ExitCode, CliError> {
    let mut choices = interactive::build_agent_choices(&registry);
    let stdin = io::stdin();
    let mut input = stdin.lock();

    {
        let mut out = io::stdout().lock();
        interactive::render_line_startup_home(&mut out)?;
    }

    let Some(first_line) = interactive::read_startup_line(&mut input)? else {
        return Ok(ExitCode::from(2));
    };
    let initial_action = interactive::parse_home_action(&first_line, &choices);
    let mut initial_agent = match initial_action {
        interactive::HomeAction::PickAgent => {
            let mut out = io::stdout().lock();
            interactive::select_agent_line(&mut input, &mut out, &choices, None)?
        }
        interactive::HomeAction::LaunchAgent(name) => choices
            .iter()
            .find(|choice| choice.name == name && choice.is_available())
            .cloned(),
        interactive::HomeAction::Quit => return Ok(ExitCode::SUCCESS),
        interactive::HomeAction::Help => {
            let mut out = io::stdout().lock();
            interactive::render_help(&mut out)?;
            None
        }
        interactive::HomeAction::Submit(_) | interactive::HomeAction::Empty => {
            eprintln!(
                "orchester: enter `/agent` or `/codex` to choose a delegate; use `orchester run --agent <name> <prompt>` for scripts"
            );
            return Ok(ExitCode::from(2));
        }
    };

    let Some(mut agent) = initial_agent.take() else {
        return Ok(ExitCode::SUCCESS);
    };

    let conductor = Conductor::new(registry);
    let mut sessions: HashMap<String, String> = HashMap::new();

    loop {
        let resume = agent
            .supports_resume
            .then(|| sessions.get(&agent.name).map(String::as_str))
            .flatten();

        let action = {
            let mut out = io::stdout().lock();
            interactive::read_prompt_action(&mut input, &mut out, &agent, resume, &choices)?
        };

        match action {
            PromptAction::Run(prompt) => {
                let resume = resume.map(str::to_owned);
                {
                    let mut out = io::stdout().lock();
                    interactive::render_run_header(&mut out, &agent, resume.as_deref())?;
                }

                let (record_task, result) =
                    match drive_agent_run(&conductor, &agent.name, prompt, resume, None, false)
                        .await
                    {
                        Ok(run) => run,
                        Err(e) => {
                            eprintln!("orchester: {e}");
                            continue;
                        }
                    };

                if let Err(e) = record_session(&agent.name, &record_task, &result) {
                    eprintln!("orchester: failed to record session metadata: {e}");
                }
                if agent.supports_resume {
                    if let Some(session_id) = result.session_id.clone() {
                        sessions.insert(agent.name.clone(), session_id);
                    }
                }

                let mut out = io::stdout().lock();
                interactive::render_run_footer(
                    &mut out,
                    result.outcome,
                    result.usage.input_tokens,
                    result.usage.output_tokens,
                )?;
            }
            PromptAction::PickAgent => {
                choices = interactive::build_agent_choices(conductor.registry());
                let maybe_agent = {
                    let mut out = io::stdout().lock();
                    interactive::select_agent_line(&mut input, &mut out, &choices, None)?
                };
                if let Some(next_agent) = maybe_agent {
                    agent = next_agent;
                }
            }
            PromptAction::LaunchAgent(name) => {
                choices = interactive::build_agent_choices(conductor.registry());
                if let Some(next_agent) = choices.iter().find(|choice| choice.name == name) {
                    agent = next_agent.clone();
                    if agent.native_command.is_some()
                        && io::stdin().is_terminal()
                        && io::stdout().is_terminal()
                        && launch_native_agent(&agent)? == NativeLaunchStatus::Cancelled
                    {
                        return Ok(ExitCode::from(130));
                    }
                } else {
                    eprintln!("orchester: unknown agent `{name}`");
                }
            }
            PromptAction::ListAgents => {
                choices = interactive::build_agent_choices(conductor.registry());
                let mut out = io::stdout().lock();
                interactive::render_agent_table(&mut out, &choices, Some(agent.name.as_str()))?;
            }
            PromptAction::Help => {
                let mut out = io::stdout().lock();
                interactive::render_help(&mut out)?;
            }
            PromptAction::Quit => return Ok(ExitCode::SUCCESS),
            PromptAction::Empty => {}
        }
    }
}

async fn run_adapter_prompt_shell(
    registry: &Registry,
    mut agent: AgentChoice,
) -> Result<(), CliError> {
    let mut choices = interactive::build_agent_choices(registry);
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let conductor = Conductor::new(registry.clone());
    let mut sessions: HashMap<String, String> = HashMap::new();

    loop {
        let resume = agent
            .supports_resume
            .then(|| sessions.get(&agent.name).map(String::as_str))
            .flatten();
        let action = {
            let mut out = io::stdout().lock();
            interactive::read_prompt_action(&mut input, &mut out, &agent, resume, &choices)?
        };

        match action {
            PromptAction::Run(prompt) => {
                let resume = resume.map(str::to_owned);
                {
                    let mut out = io::stdout().lock();
                    interactive::render_run_header(&mut out, &agent, resume.as_deref())?;
                }
                let (record_task, result) =
                    match drive_agent_run(&conductor, &agent.name, prompt, resume, None, false)
                        .await
                    {
                        Ok(run) => run,
                        Err(e) => {
                            eprintln!("orchester: {e}");
                            continue;
                        }
                    };
                if let Err(e) = record_session(&agent.name, &record_task, &result) {
                    eprintln!("orchester: failed to record session metadata: {e}");
                }
                if let Some(session_id) = result.session_id.clone() {
                    sessions.insert(agent.name.clone(), session_id);
                }
                let mut out = io::stdout().lock();
                interactive::render_run_footer(
                    &mut out,
                    result.outcome,
                    result.usage.input_tokens,
                    result.usage.output_tokens,
                )?;
            }
            PromptAction::PickAgent => return Ok(()),
            PromptAction::LaunchAgent(name) => {
                choices = interactive::build_agent_choices(registry);
                if let Some(next_agent) = choices.iter().find(|choice| choice.name == name) {
                    agent = next_agent.clone();
                    if agent.native_command.is_some() {
                        return Ok(());
                    }
                }
            }
            PromptAction::ListAgents => {
                choices = interactive::build_agent_choices(registry);
                let mut out = io::stdout().lock();
                interactive::render_agent_table(&mut out, &choices, Some(agent.name.as_str()))?;
            }
            PromptAction::Help => {
                let mut out = io::stdout().lock();
                interactive::render_help(&mut out)?;
            }
            PromptAction::Quit => return Ok(()),
            PromptAction::Empty => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeLaunchStatus {
    Completed,
    Cancelled,
}

#[derive(Debug)]
struct NativeInvocation {
    program: PathBuf,
    args: Vec<OsString>,
    envs: Vec<(OsString, OsString)>,
}

fn launch_native_agent(agent: &AgentChoice) -> Result<NativeLaunchStatus, CliError> {
    let Some(command) = agent.native_command.as_deref() else {
        return Err(CliError::NativeAgentUnavailable(agent.name.clone()));
    };
    if !agent.is_available() {
        return Err(CliError::NativeAgentUnavailable(agent.name.clone()));
    }

    let executable = resolve_native_command(command)
        .ok_or_else(|| CliError::NativeAgentUnavailable(agent.name.clone()))?;
    let invocation = native_invocation(&executable, Vec::new());

    println!(
        "\x1b[2mLaunching {} ({})...\x1b[0m",
        agent.name,
        invocation.program.display()
    );
    let mut process = ProcessCommand::new(&invocation.program);
    process.args(&invocation.args);
    for (key, value) in &invocation.envs {
        process.env(key, value);
    }
    let status = process
        .current_dir(std::env::current_dir()?)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if is_cancelled_status(&status) {
        return Ok(NativeLaunchStatus::Cancelled);
    }
    if !status.success() {
        eprintln!("orchester: {} exited with {}", agent.name, status);
    }
    Ok(NativeLaunchStatus::Completed)
}

#[cfg(windows)]
fn native_invocation(executable: &Path, extra_args: Vec<OsString>) -> NativeInvocation {
    if is_windows_shell_script(executable) {
        if let Some(invocation) = windows_cmd_shim_invocation(executable, &extra_args) {
            return invocation;
        }
        if let Some(ps1) = adjacent_powershell_shim(executable) {
            return powershell_invocation(&ps1, extra_args);
        }
        let mut args = vec![
            OsString::from("/d"),
            OsString::from("/c"),
            executable.as_os_str().to_os_string(),
        ];
        args.extend(extra_args);
        return NativeInvocation {
            program: PathBuf::from("cmd.exe"),
            args,
            envs: Vec::new(),
        };
    }

    NativeInvocation {
        program: executable.to_path_buf(),
        args: extra_args,
        envs: Vec::new(),
    }
}

#[cfg(not(windows))]
fn native_invocation(executable: &Path, extra_args: Vec<OsString>) -> NativeInvocation {
    NativeInvocation {
        program: executable.to_path_buf(),
        args: extra_args,
        envs: Vec::new(),
    }
}

fn resolve_native_command(command: &str) -> Option<PathBuf> {
    let command_path = Path::new(command);
    if command_path.components().count() > 1 || command_path.is_absolute() {
        return command_path.is_file().then(|| command_path.to_path_buf());
    }

    let path = env::var_os("PATH")?;
    let names = executable_names(command);
    for dir in env::split_paths(&path) {
        for name in &names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
fn executable_names(command: &str) -> Vec<OsString> {
    if Path::new(command).extension().is_some() {
        return vec![OsString::from(command)];
    }

    let mut names = Vec::new();
    if let Some(pathext) = env::var_os("PATHEXT") {
        for ext in env::split_paths(&pathext) {
            if let Some(ext) = ext.to_str() {
                names.push(OsString::from(format!("{command}{ext}")));
            }
        }
    } else {
        for ext in [".COM", ".EXE", ".BAT", ".CMD"] {
            names.push(OsString::from(format!("{command}{ext}")));
        }
    }
    names.push(OsString::from(command));
    names
}

#[cfg(not(windows))]
fn executable_names(command: &str) -> Vec<OsString> {
    vec![OsString::from(command)]
}

#[cfg(windows)]
fn is_windows_shell_script(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
        .unwrap_or(false)
}

#[cfg(windows)]
fn adjacent_powershell_shim(executable: &Path) -> Option<PathBuf> {
    let mut ps1 = executable.to_path_buf();
    ps1.set_extension("ps1");
    ps1.is_file().then_some(ps1)
}

#[cfg(windows)]
fn powershell_invocation(script: &Path, extra_args: Vec<OsString>) -> NativeInvocation {
    let mut args = vec![
        OsString::from("-NoProfile"),
        OsString::from("-ExecutionPolicy"),
        OsString::from("Bypass"),
        OsString::from("-File"),
        script.as_os_str().to_os_string(),
    ];
    args.extend(extra_args);
    NativeInvocation {
        program: PathBuf::from("powershell.exe"),
        args,
        envs: Vec::new(),
    }
}

#[cfg(windows)]
fn windows_cmd_shim_invocation(
    executable: &Path,
    extra_args: &[OsString],
) -> Option<NativeInvocation> {
    let content = fs::read_to_string(executable).ok()?;
    let dir = executable.parent()?;

    if let Some(invocation) = opencode_cmd_invocation(&content, dir, extra_args) {
        return Some(invocation);
    }

    let script = find_node_script_entry(&content, dir)?;
    let program = local_node_or_path_node(dir);
    let mut args = vec![script.into_os_string()];
    args.extend(extra_args.iter().cloned());
    Some(NativeInvocation {
        program,
        args,
        envs: node_path_env(&content),
    })
}

#[cfg(windows)]
fn opencode_cmd_invocation(
    content: &str,
    dir: &Path,
    extra_args: &[OsString],
) -> Option<NativeInvocation> {
    let exe = content
        .lines()
        .find_map(|line| parse_cmd_set(line, "opencode_exe"))
        .map(|value| expand_cmd_path(&value, dir))?;
    if !exe.is_file() {
        return None;
    }

    let mut args = Vec::new();
    if extra_args.is_empty() {
        args.extend(
            ["web", "--hostname", "127.0.0.1", "--port", "4096"]
                .into_iter()
                .map(OsString::from),
        );
    } else {
        args.extend(extra_args.iter().cloned());
    }

    Some(NativeInvocation {
        program: exe,
        args,
        envs: Vec::new(),
    })
}

#[cfg(windows)]
fn find_node_script_entry(content: &str, dir: &Path) -> Option<PathBuf> {
    content.lines().find_map(|line| {
        if !line.to_ascii_lowercase().contains(".js") {
            return None;
        }
        quoted_tokens(line).into_iter().find_map(|token| {
            if !token.to_ascii_lowercase().contains(".js") {
                return None;
            }
            let path = expand_cmd_path(&token, dir);
            path.is_file().then_some(path)
        })
    })
}

#[cfg(windows)]
fn local_node_or_path_node(dir: &Path) -> PathBuf {
    let local = dir.join("node.exe");
    if local.is_file() {
        local
    } else {
        PathBuf::from("node")
    }
}

#[cfg(windows)]
fn node_path_env(content: &str) -> Vec<(OsString, OsString)> {
    let Some(new_path) = content.lines().find_map(|line| {
        let value = parse_cmd_set(line, "NODE_PATH")?;
        (!value.contains("%NODE_PATH%")).then_some(value)
    }) else {
        return Vec::new();
    };

    let value = match env::var_os("NODE_PATH") {
        Some(existing) if !existing.is_empty() => {
            OsString::from(format!("{new_path};{}", existing.to_string_lossy()))
        }
        _ => OsString::from(new_path),
    };
    vec![(OsString::from("NODE_PATH"), value)]
}

#[cfg(windows)]
fn parse_cmd_set(line: &str, var: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('@').trim();
    let prefix = format!("SET \"{}=", var.to_ascii_uppercase());
    let upper = trimmed.to_ascii_uppercase();
    if !upper.starts_with(&prefix) {
        return None;
    }
    let mut value = trimmed[prefix.len()..].to_string();
    if value.ends_with('"') {
        value.pop();
    }
    Some(value)
}

#[cfg(windows)]
fn quoted_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current: Option<String> = None;
    for ch in line.chars() {
        if ch == '"' {
            if let Some(token) = current.take() {
                tokens.push(token);
            } else {
                current = Some(String::new());
            }
        } else if let Some(token) = current.as_mut() {
            token.push(ch);
        }
    }
    tokens
}

#[cfg(windows)]
fn expand_cmd_path(raw: &str, dir: &Path) -> PathBuf {
    let dir = dir.to_string_lossy();
    let expanded = raw
        .replace("%dp0%", &dir)
        .replace("%DP0%", &dir)
        .replace("%~dp0", &dir)
        .replace("%~DP0", &dir);
    PathBuf::from(expanded)
}

fn is_cancelled_status(status: &ExitStatus) -> bool {
    #[cfg(windows)]
    {
        const STATUS_CONTROL_C_EXIT: i32 = 0xC000_013A_u32 as i32;
        matches!(status.code(), Some(130) | Some(STATUS_CONTROL_C_EXIT))
    }

    #[cfg(not(windows))]
    {
        status.code() == Some(130)
    }
}

async fn drive_agent_run(
    conductor: &Conductor,
    agent: &str,
    prompt: String,
    resume: Option<String>,
    model: Option<String>,
    json_mode: bool,
) -> Result<(Task, RunResult), CliError> {
    let mut task = Task::new(prompt, PathBuf::from("."));
    if let Some(id) = resume {
        task = task.with_resume(id);
    }
    if let Some(model) = model {
        task = task.with_model(model);
    }
    let record_task = task.clone();

    // Render live while folding into a RunResult.
    let result = conductor
        .run_to_result(agent, task, |event| {
            let mut out = io::stdout().lock();
            let r = if json_mode {
                render::render_event_json(&mut out, event)
            } else {
                render::render_event(&mut out, event)
            };
            // A broken pipe (e.g. `| head`) shouldn't panic the run.
            let _ = r.and_then(|_| out.flush());
        })
        .await?;

    Ok((record_task, result))
}

fn record_session(agent: &str, task: &Task, result: &RunResult) -> io::Result<()> {
    session_store().append(&SessionRecord::new(agent, task, result))
}

fn session_store() -> SessionStore {
    SessionStore::new(orchester_home().join("sessions.jsonl"))
}

fn discover_registry() -> Result<Registry, CliError> {
    let project_directory = std::env::current_dir()?;
    let plugin_roots = standard_plugin_roots(orchester_home(), &project_directory)?;
    Ok(Registry::discover_with_plugin_roots(
        project_directory.join(MANIFEST_DIR),
        plugin_roots,
    ))
}

fn orchester_home() -> PathBuf {
    if let Some(path) = std::env::var_os("ORCHESTER_HOME") {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(path).join("Orchester");
    }
    if let Some(path) = std::env::var_os("USERPROFILE") {
        return PathBuf::from(path).join(".orchester");
    }
    if let Some(path) = std::env::var_os("HOME") {
        return PathBuf::from(path).join(".orchester");
    }
    PathBuf::from(".orchester")
}

/// Resolve the prompt argument: `-` (or absent with piped stdin) reads stdin.
fn read_prompt(arg: Option<String>) -> Result<String, CliError> {
    match arg.as_deref() {
        Some("-") | None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf).map_err(CliError::Io)?;
            let trimmed = buf.trim().to_string();
            if trimmed.is_empty() {
                Err(CliError::MissingPrompt)
            } else {
                Ok(trimmed)
            }
        }
        Some(p) => Ok(p.to_string()),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn windows_node_shim_invocation_bypasses_cmd() {
        let dir = temp_dir("node-shim");
        fs::create_dir_all(dir.join("node_modules/pkg/bin")).unwrap();
        let script = dir.join("node_modules/pkg/bin/cli.js");
        fs::write(&script, "").unwrap();
        let shim = dir.join("tool.cmd");
        fs::write(
            &shim,
            r#"@ECHO off
SETLOCAL
node "%dp0%\node_modules\pkg\bin\cli.js" %*
"#,
        )
        .unwrap();

        let invocation =
            windows_cmd_shim_invocation(&shim, &[OsString::from("--version")]).unwrap();

        assert_eq!(invocation.program, PathBuf::from("node"));
        assert_eq!(
            fs::canonicalize(PathBuf::from(&invocation.args[0])).unwrap(),
            fs::canonicalize(script).unwrap()
        );
        assert_eq!(invocation.args[1], OsString::from("--version"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn windows_opencode_shim_preserves_no_arg_web_default() {
        let dir = temp_dir("opencode-shim");
        fs::create_dir_all(dir.join("node_modules/opencode-ai/bin")).unwrap();
        let exe = dir.join("node_modules/opencode-ai/bin/opencode.exe");
        fs::write(&exe, "").unwrap();
        let shim = dir.join("opencode.cmd");
        fs::write(
            &shim,
            r#"@ECHO off
SET "opencode_exe=%dp0%\node_modules\opencode-ai\bin\opencode.exe"
"#,
        )
        .unwrap();

        let invocation = windows_cmd_shim_invocation(&shim, &[]).unwrap();

        assert_eq!(invocation.program, exe);
        assert_eq!(
            invocation.args,
            vec![
                OsString::from("web"),
                OsString::from("--hostname"),
                OsString::from("127.0.0.1"),
                OsString::from("--port"),
                OsString::from("4096"),
            ]
        );
        fs::remove_dir_all(dir).ok();
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("orchester-{name}-{}-{nanos}", std::process::id()))
    }
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("no agent given; pass --agent <name> (or run `orchester list`)")]
    MissingAgent,
    #[error("no prompt given; pass a prompt argument or `-` to read stdin")]
    MissingPrompt,
    #[error("agent `{0}` is not available as a native interactive CLI")]
    NativeAgentUnavailable(String),
    #[error(transparent)]
    Conductor(#[from] ConductorError),
    #[error(transparent)]
    PluginRoot(#[from] PluginRootError),
    #[error(transparent)]
    Io(#[from] io::Error),
}
