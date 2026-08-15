//! `orchester` — the CLI entry point.
//!
//! Flow: parse args → discover adapters into a [`Registry`] → either print the
//! adapter list, or build a [`Task`] and drive it through the [`Conductor`],
//! rendering each event (or emitting Event JSONL under `--json`). The process
//! exit code reflects the run outcome so scripts can branch on success/failure.

mod args;
mod avatar;
mod interactive;
mod plugin;
mod process;
mod render;
mod self_agent;

use std::collections::HashMap;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::process::{Command as ProcessCommand, Stdio};

use clap::Parser;

use orchester_laufzeit::{Conductor, ConductorError, SessionRecord, SessionStore};
use orchester_protokoll::{Outcome, RunResult, Task};
use orchester_verzeichnis::{standard_plugin_roots, PluginRootError, Registry};

use args::{Cli, Command, PluginCommand, PluginInstallArgs, PluginRemoveArgs, PluginStatusArgs};
use interactive::{
    AgentChoice, ChatHomeView, CredentialCommand, ModelCommand, PluginAction, PromptAction,
    TranscriptEntry, WorkspaceCommand,
};
use process::{command_invocation, is_cancelled_status, resolve_command};
use self_agent::{SelfAgentHost, SelfAgentHostError};
use tokio_util::sync::CancellationToken;

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
        Some(Command::Plugin(plugin_args)) => {
            return plugin::run(&registry, plugin_args.command, json, &orchester_home())
                .map(ExitCode::from)
                .map_err(CliError::Io);
        }
        Some(Command::Config) => {
            let mut self_agent = self_agent_host()?;
            render_workspace_command(&mut self_agent, WorkspaceCommand::Config)?;
            return Ok(ExitCode::SUCCESS);
        }
        Some(Command::Login(args)) => {
            let mut self_agent = self_agent_host()?;
            render_workspace_command(
                &mut self_agent,
                WorkspaceCommand::Credential(CredentialCommand::Login {
                    provider: args.provider,
                }),
            )?;
            return Ok(ExitCode::SUCCESS);
        }
        Some(Command::Logout(args)) => {
            let mut self_agent = self_agent_host()?;
            render_workspace_command(
                &mut self_agent,
                WorkspaceCommand::Credential(CredentialCommand::Logout {
                    provider: args.provider,
                }),
            )?;
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

async fn run_terminal_interactive(mut registry: Registry) -> Result<ExitCode, CliError> {
    let mut self_agent = self_agent_host()?;
    let mut chat = interactive::ChatSession::enter()?;
    let mut transcript = Vec::new();
    let mut input = String::new();
    let mut command_selected = 0usize;
    let mut show_help = false;
    let mut busy = None;

    loop {
        let choices = interactive::build_agent_choices(&registry);
        let model_status = self_agent
            .model_label()
            .unwrap_or_else(|_| "model unavailable".into());
        chat.present_view(ChatHomeView::new(
            &input,
            &choices,
            command_selected,
            show_help,
            &model_status,
            &transcript,
            busy.as_deref(),
        ))?;

        let Some(key) = chat.read_key()? else {
            continue;
        };
        let Some(action) = interactive::handle_chat_key(
            key,
            &mut input,
            &mut command_selected,
            &mut show_help,
            &choices,
        ) else {
            continue;
        };

        match action {
            interactive::HomeAction::Quit => return Ok(ExitCode::SUCCESS),
            interactive::HomeAction::Submit(prompt) => {
                input.clear();
                command_selected = 0;
                show_help = false;
                transcript.push(TranscriptEntry::user(&prompt));
                busy = Some("Creating...".to_owned());
                chat.present_view(ChatHomeView::new(
                    &input,
                    &choices,
                    command_selected,
                    show_help,
                    &model_status,
                    &transcript,
                    busy.as_deref(),
                ))?;

                match self_agent.submit(prompt, CancellationToken::new()).await {
                    Ok(outcome) => {
                        let rendered = self_agent::render_outcome_transcript(&outcome)?;
                        if rendered.is_empty() {
                            transcript.push(TranscriptEntry::status("No response returned."));
                        } else {
                            transcript.push(TranscriptEntry::assistant(rendered));
                        }
                    }
                    Err(error) => transcript.push(TranscriptEntry::error(error.to_string())),
                }
                busy = None;
            }
            interactive::HomeAction::Workspace(command) => {
                let label = format!("/{command:?}");
                drop(chat);
                let result = render_workspace_command(&mut self_agent, command);
                chat = interactive::ChatSession::enter()?;
                match result {
                    Ok(()) => {
                        transcript.push(TranscriptEntry::status(format!("{label} completed")))
                    }
                    Err(error) => transcript.push(TranscriptEntry::error(error.to_string())),
                }
            }
            interactive::HomeAction::Empty => {}
            interactive::HomeAction::PickAgent => {
                drop(chat);
                let selected = interactive::select_agent_tui(&choices, None)?;
                chat = interactive::ChatSession::enter()?;
                if let Some(agent) = selected {
                    if agent.native_command.is_some() {
                        drop(chat);
                        let status = launch_native_agent(&agent)?;
                        chat = interactive::ChatSession::enter()?;
                        if status == NativeLaunchStatus::Cancelled {
                            return Ok(ExitCode::from(130));
                        }
                    } else {
                        drop(chat);
                        let result = run_adapter_prompt_shell(&registry, agent).await;
                        chat = interactive::ChatSession::enter()?;
                        match result {
                            Ok(()) => registry = discover_registry()?,
                            Err(error) => {
                                transcript.push(TranscriptEntry::error(error.to_string()))
                            }
                        }
                    }
                }
            }
            interactive::HomeAction::LaunchAgent(name) => {
                let Some(agent) = choices.iter().find(|choice| choice.name == name) else {
                    transcript.push(TranscriptEntry::error(format!(
                        "unknown or unavailable agent `{name}`"
                    )));
                    continue;
                };
                if agent.native_command.is_some() {
                    drop(chat);
                    let status = launch_native_agent(agent)?;
                    chat = interactive::ChatSession::enter()?;
                    if status == NativeLaunchStatus::Cancelled {
                        return Ok(ExitCode::from(130));
                    }
                } else {
                    drop(chat);
                    let result = run_adapter_prompt_shell(&registry, agent.clone()).await;
                    chat = interactive::ChatSession::enter()?;
                    match result {
                        Ok(()) => registry = discover_registry()?,
                        Err(error) => transcript.push(TranscriptEntry::error(error.to_string())),
                    }
                }
            }
            interactive::HomeAction::Plugins(action) => {
                drop(chat);
                let result =
                    plugin::run(&registry, plugin_command(action), false, &orchester_home());
                chat = interactive::ChatSession::enter()?;
                match result {
                    Ok(outcome) if outcome.failed() => {
                        transcript.push(TranscriptEntry::error("plugin command failed"))
                    }
                    Ok(_) => registry = discover_registry()?,
                    Err(error) => transcript.push(TranscriptEntry::error(error.to_string())),
                }
            }
            interactive::HomeAction::Help => show_help = true,
        }
    }
}

async fn run_line_interactive(registry: Registry) -> Result<ExitCode, CliError> {
    run_line_interactive_with_host(registry, self_agent_host()?, true).await
}

async fn run_line_interactive_with_host(
    mut registry: Registry,
    mut self_agent: SelfAgentHost,
    render_startup: bool,
) -> Result<ExitCode, CliError> {
    let mut choices = interactive::build_agent_choices(&registry);
    let stdin = io::stdin();
    let mut input = stdin.lock();

    if render_startup {
        let mut out = io::stdout().lock();
        let model_status = self_agent
            .model_label()
            .unwrap_or_else(|_| "model unavailable".into());
        interactive::render_line_startup_home(&mut out, &model_status)?;
    } else {
        let mut out = io::stdout().lock();
        interactive::render_line_continue_prompt(&mut out)?;
    }

    let mut received_input = false;
    let mut initial_agent = loop {
        let Some(line) = interactive::read_startup_line(&mut input)? else {
            return Ok(if received_input {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            });
        };
        received_input = true;
        match interactive::parse_home_action(&line, &choices) {
            interactive::HomeAction::PickAgent => {
                let mut out = io::stdout().lock();
                break interactive::select_agent_line(&mut input, &mut out, &choices, None)?;
            }
            interactive::HomeAction::LaunchAgent(name) => {
                break choices
                    .iter()
                    .find(|choice| choice.name == name && choice.is_available())
                    .cloned();
            }
            interactive::HomeAction::Quit => return Ok(ExitCode::SUCCESS),
            interactive::HomeAction::Help => {
                let mut out = io::stdout().lock();
                interactive::render_help(&mut out)?;
                interactive::render_line_continue_prompt(&mut out)?;
            }
            interactive::HomeAction::Plugins(action) => {
                let outcome =
                    plugin::run(&registry, plugin_command(action), false, &orchester_home())?;
                // A mutation that was rejected must still reach the caller as a
                // failing exit status, so only a successful command returns to
                // the prompt.
                if outcome.failed() {
                    return Ok(ExitCode::from(outcome));
                }
                // Install and remove change what is on disk, so the home must
                // re-read it rather than keep serving the startup snapshot.
                registry = discover_registry()?;
                choices = interactive::build_agent_choices(&registry);
                let mut out = io::stdout().lock();
                interactive::render_line_continue_prompt(&mut out)?;
            }
            interactive::HomeAction::Workspace(command) => {
                render_workspace_command(&mut self_agent, command)?;
                let mut out = io::stdout().lock();
                interactive::render_line_continue_prompt(&mut out)?;
            }
            interactive::HomeAction::Submit(prompt) => {
                let outcome = self_agent.submit(prompt, CancellationToken::new()).await?;
                let mut out = io::stdout().lock();
                self_agent::render_outcome(&mut out, &outcome)?;
                interactive::render_line_continue_prompt(&mut out)?;
            }
            interactive::HomeAction::Empty => return Ok(ExitCode::from(2)),
        }
    };

    let Some(mut agent) = initial_agent.take() else {
        return Ok(ExitCode::SUCCESS);
    };

    let mut conductor = Conductor::new(registry);
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
            PromptAction::Workspace(command) => {
                render_workspace_command(&mut self_agent, command)?;
            }
            PromptAction::Plugins(action) => {
                let _ = plugin::run(
                    conductor.registry(),
                    plugin_command(action),
                    false,
                    &orchester_home(),
                )?;
                conductor = Conductor::new(discover_registry()?);
                choices = interactive::build_agent_choices(conductor.registry());
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
    let mut conductor = Conductor::new(registry.clone());
    let mut sessions: HashMap<String, String> = HashMap::new();
    let mut self_agent = self_agent_host()?;

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
                choices = interactive::build_agent_choices(conductor.registry());
                if let Some(next_agent) = choices.iter().find(|choice| choice.name == name) {
                    agent = next_agent.clone();
                    if agent.native_command.is_some() {
                        return Ok(());
                    }
                }
            }
            PromptAction::ListAgents => {
                choices = interactive::build_agent_choices(conductor.registry());
                let mut out = io::stdout().lock();
                interactive::render_agent_table(&mut out, &choices, Some(agent.name.as_str()))?;
            }
            PromptAction::Workspace(command) => {
                render_workspace_command(&mut self_agent, command)?;
            }
            PromptAction::Plugins(action) => {
                let _ = plugin::run(
                    conductor.registry(),
                    plugin_command(action),
                    false,
                    &orchester_home(),
                )?;
                conductor = Conductor::new(discover_registry()?);
                choices = interactive::build_agent_choices(conductor.registry());
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

fn launch_native_agent(agent: &AgentChoice) -> Result<NativeLaunchStatus, CliError> {
    let Some(command) = agent.native_command.as_deref() else {
        return Err(CliError::NativeAgentUnavailable(agent.name.clone()));
    };
    if !agent.is_available() {
        return Err(CliError::NativeAgentUnavailable(agent.name.clone()));
    }

    let executable = resolve_command(command)
        .ok_or_else(|| CliError::NativeAgentUnavailable(agent.name.clone()))?;
    let invocation = command_invocation(&executable, Vec::new());

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

fn plugin_command(action: PluginAction) -> PluginCommand {
    match action {
        PluginAction::List => PluginCommand::List,
        PluginAction::Status(name) => PluginCommand::Status(PluginStatusArgs { name }),
        PluginAction::Install(name) => PluginCommand::Install(PluginInstallArgs { name }),
        PluginAction::Remove(name) => PluginCommand::Remove(PluginRemoveArgs { name }),
    }
}

/// The Orchester home, resolved by the runtime so the CLI and the config
/// loader can never disagree about where Orchester keeps its files.
///
/// A home that cannot be resolved stays relative on purpose: the plugin layer
/// rejects it with a message that does not echo the offending value.
fn orchester_home() -> PathBuf {
    orchester_laufzeit::harness::orchester_home()
        .unwrap_or_else(|| PathBuf::from(orchester_laufzeit::harness::ORCHESTER_DIR))
}

fn self_agent_host() -> Result<SelfAgentHost, io::Error> {
    let state_root = orchester_home().join("state");
    Ok(SelfAgentHost::new(
        std::env::current_dir()?,
        state_root.join("runs.db"),
        state_root.join("audit.jsonl"),
    ))
}

fn render_workspace_command(
    self_agent: &mut SelfAgentHost,
    command: WorkspaceCommand,
) -> Result<(), CliError> {
    match command {
        WorkspaceCommand::Status => {
            let status = self_agent.status()?;
            let mut out = io::stdout().lock();
            self_agent::render_status(&mut out, &status)?;
        }
        WorkspaceCommand::Config => {
            let view = self_agent.config_view()?;
            let mut out = io::stdout().lock();
            self_agent::render_config(&mut out, &view)?;
        }
        WorkspaceCommand::Permissions => {
            let permissions = self_agent.permissions()?;
            let mut out = io::stdout().lock();
            self_agent::render_permissions(&mut out, &permissions)?;
        }
        WorkspaceCommand::Resume => {
            let catalog = self_agent.resume_catalog()?;
            let mut out = io::stdout().lock();
            self_agent::render_resume(&mut out, &catalog)?;
        }
        WorkspaceCommand::Model(ModelCommand::Show) => {
            let models = self_agent.model_catalog()?;
            let mut out = io::stdout().lock();
            self_agent::render_models(&mut out, &models)?;
        }
        WorkspaceCommand::Model(ModelCommand::SelectProfile(name)) => {
            let selected = self_agent.select_model_profile(&name)?;
            let mut out = io::stdout().lock();
            self_agent::render_model_selection(&mut out, &selected)?;
        }
        WorkspaceCommand::Model(ModelCommand::UseConfigured) => {
            let selected = self_agent.select_configured_model()?;
            let mut out = io::stdout().lock();
            self_agent::render_model_selection(&mut out, &selected)?;
        }
        WorkspaceCommand::Credential(CredentialCommand::Login { provider }) => {
            let target = self_agent.credential_target(provider.as_deref())?;
            {
                let mut out = io::stdout().lock();
                self_agent::render_credential_target(&mut out, &target)?;
            }
            // The prompt writes to the same terminal, so the stdout lock is
            // released before asking and retaken to confirm.
            let Some(secret) = interactive::prompt_secret("API key")? else {
                let mut out = io::stdout().lock();
                writeln!(out, "cancelled; nothing was stored")?;
                return Ok(());
            };
            let (update, wiring, config_path) = self_agent.store_credential(&target, secret)?;
            let mut out = io::stdout().lock();
            self_agent::render_credential_stored(&mut out, &update, &wiring, &config_path)?;
        }
        WorkspaceCommand::Credential(CredentialCommand::Logout { provider }) => {
            let target = self_agent.credential_target(provider.as_deref())?;
            let removed = self_agent.clear_credential(&target.provider)?;
            let mut out = io::stdout().lock();
            self_agent::render_credential_cleared(&mut out, &target.provider, removed)?;
        }
    }
    Ok(())
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
    SelfAgent(#[from] SelfAgentHostError),
    #[error(transparent)]
    Io(#[from] io::Error),
}
