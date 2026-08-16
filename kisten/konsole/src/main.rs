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
mod theme;
mod workspace_overlay;

use std::collections::{HashMap, VecDeque};
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::Parser;

use orchester_laufzeit::harness::service::{
    SelfAgentActiveModel, SelfAgentModelCatalog, SelfAgentModelChoice, SelfAgentResumeAvailability,
    SelfAgentResumeCatalog, SelfAgentRunOutcome,
};
use orchester_laufzeit::harness::StreamingRedactor;
use orchester_laufzeit::{Conductor, ConductorError, SessionRecord, SessionStore};
use orchester_modell::ModelEventSink;
use orchester_protokoll::{Outcome, RunResult, Task};
use orchester_verzeichnis::{standard_plugin_roots, PluginRootError, Registry};

use args::{Cli, Command, PluginCommand, PluginInstallArgs, PluginRemoveArgs, PluginStatusArgs};
use interactive::{
    AgentChoice, ChatHomeView, CommandOverlay, CredentialCommand, ModelCommand, OverlayInput,
    OverlayItem, PluginAction, PromptAction, ThemeCommand, TranscriptEntry, WorkspaceCommand,
};
use process::{command_invocation, is_cancelled_status, resolve_command};
use self_agent::{SelfAgentHost, SelfAgentHostError};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use workspace_overlay::WorkspaceInspection;

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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelTurnRequest {
    Prompt(String),
    Resume(String),
}

enum QueuedChatAction {
    Turn(ModelTurnRequest),
    Command(interactive::HomeAction),
}

#[derive(Debug, Clone)]
enum OverlayAction {
    Close,
    Inspect(Vec<String>),
    Resume(String),
    ModelConfigured,
    ModelProfile(String),
    ModelEffort {
        target: ModelSelectionTarget,
        effort: Option<String>,
    },
    Theme(theme::Theme),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelSelectionTarget {
    Configured,
    Profile(String),
}

#[derive(Debug, Clone)]
struct TerminalOverlay {
    view: CommandOverlay,
    actions: Vec<OverlayAction>,
    parent: Option<Box<TerminalOverlay>>,
}

impl TerminalOverlay {
    fn report(label: &str, output: &str) -> Self {
        let cleaned = interactive::clean_transcript_text(output);
        let rows = cleaned
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let mut items = rows
            .iter()
            .map(|line| report_overlay_item(line))
            .collect::<Vec<_>>();
        if items.is_empty() {
            items.push(OverlayItem::new("No results", ""));
        }
        let mut actions = rows
            .into_iter()
            .map(|line| OverlayAction::Inspect(vec![line.to_owned()]))
            .collect::<Vec<_>>();
        if actions.is_empty() {
            actions.push(OverlayAction::Inspect(vec!["No results".into()]));
        }
        Self {
            view: CommandOverlay::new(
                format!("{label} result"),
                "This result remains visible until you close it.",
                items,
            )
            .with_footer("Up/Down select  |  Enter inspect  |  Esc close"),
            actions,
            parent: None,
        }
    }

    fn inspect(&mut self, index: usize) -> bool {
        let Some(OverlayAction::Inspect(details)) = self.actions.get(index) else {
            return false;
        };
        self.view.details = details.clone();
        true
    }

    fn error(label: &str, error: &impl std::fmt::Display) -> Self {
        Self::report(label, &format!("Error\n{error}"))
    }

    fn inspection(inspection: WorkspaceInspection) -> Self {
        let WorkspaceInspection {
            title,
            description,
            entries,
            footer,
        } = inspection;
        let mut items = Vec::with_capacity(entries.len().max(1));
        let mut actions = Vec::with_capacity(entries.len().max(1));
        for entry in entries {
            items.push(OverlayItem::new(entry.label, entry.detail).current(entry.current));
            actions.push(OverlayAction::Inspect(entry.details));
        }
        if items.is_empty() {
            items.push(OverlayItem::new("No results", ""));
            actions.push(OverlayAction::Inspect(vec![
                "No results are available for this workspace.".into(),
            ]));
        }
        Self {
            view: CommandOverlay::new(title, description, items).with_footer(footer),
            actions,
            parent: None,
        }
    }

    fn resume(catalog: &SelfAgentResumeCatalog) -> Self {
        let mut overlay = Self::inspection(workspace_overlay::resume(catalog));
        for (index, entry) in catalog.entries.iter().enumerate() {
            if entry.availability == SelfAgentResumeAvailability::Ready {
                overlay.actions[index] = OverlayAction::Resume(entry.handle.clone());
            }
        }
        overlay
    }

    fn models(catalog: &SelfAgentModelCatalog) -> Self {
        let current = match &catalog.active {
            SelfAgentActiveModel::Configured(choice) => {
                choice.profile.as_deref().unwrap_or("configured")
            }
            SelfAgentActiveModel::Unresolved { .. } | SelfAgentActiveModel::NotConfigured => "",
        };
        let mut items =
            vec![
                OverlayItem::new("configured", "Use the model resolved from configuration")
                    .current(current == "configured"),
            ];
        let mut actions = vec![OverlayAction::ModelConfigured];
        for choice in &catalog.profiles {
            let name = choice.profile.as_deref().unwrap_or("unnamed");
            let detail = format!(
                "{} | {} | reasoning {}",
                choice.model,
                choice.provider_name,
                choice.reasoning_effort.as_deref().unwrap_or("default")
            );
            items.push(OverlayItem::new(name, detail).current(current == name));
            actions.push(OverlayAction::ModelProfile(name.to_owned()));
        }
        Self {
            view: CommandOverlay::new(
                "Select model",
                "Choose the model used for future turns in this session.",
                items,
            )
            .with_footer("Up/Down select  |  Enter choose effort  |  Esc cancel"),
            actions,
            parent: None,
        }
    }

    fn model_efforts(target: ModelSelectionTarget, choice: &SelfAgentModelChoice) -> Self {
        const EFFORTS: [(&str, Option<&str>, &str); 6] = [
            ("default", None, "Provider default"),
            ("low", Some("low"), "Fastest reasoning"),
            ("medium", Some("medium"), "Balanced reasoning"),
            ("high", Some("high"), "Deeper reasoning"),
            ("xhigh", Some("xhigh"), "Extended reasoning"),
            ("ultra", Some("ultra"), "Maximum reasoning"),
        ];
        let current = choice.reasoning_effort.as_deref();
        let mut items = Vec::with_capacity(EFFORTS.len());
        let mut actions = Vec::with_capacity(EFFORTS.len());
        for (label, effort, detail) in EFFORTS {
            items.push(OverlayItem::new(label, detail).current(current == effort));
            actions.push(OverlayAction::ModelEffort {
                target: target.clone(),
                effort: effort.map(str::to_owned),
            });
        }
        Self {
            view: CommandOverlay::new(
                "Select reasoning effort",
                format!("{} for {}", choice.model, choice.provider_name),
                items,
            )
            .with_footer("Up/Down select  |  Enter apply  |  Esc back"),
            actions,
            parent: None,
        }
    }

    fn with_parent(mut self, parent: TerminalOverlay) -> Self {
        self.parent = Some(Box::new(parent));
        self
    }

    fn themes(current: theme::Theme) -> Self {
        let mut items = Vec::new();
        let mut actions = Vec::new();
        for candidate in theme::Theme::all() {
            items.push(
                OverlayItem::new(candidate.label(), candidate.name()).current(candidate == current),
            );
            actions.push(OverlayAction::Theme(candidate));
        }
        let mut overlay = Self {
            view: CommandOverlay::new(
                "Select theme",
                "Preview colors live; Esc restores the previous theme.",
                items,
            )
            .with_footer("Up/Down preview  |  Enter save  |  Esc cancel"),
            actions,
            parent: None,
        };
        overlay.view.selected = overlay
            .actions
            .iter()
            .position(|action| matches!(action, OverlayAction::Theme(theme) if *theme == current))
            .unwrap_or_default();
        overlay.refresh_theme_preview(current);
        overlay
    }

    fn refresh_theme_preview(&mut self, selected: theme::Theme) {
        self.view.details = vec![
            format!("Preview: {} ({})", selected.label(), selected.name()),
            "  accent: headings and active controls".into(),
            "  selection: current option and focus".into(),
            "  + assistant output remains readable".into(),
            "  - inactive text stays dimmed".into(),
        ];
    }
}

fn report_overlay_item(line: &str) -> OverlayItem {
    match line.split_once(':') {
        Some((label, detail)) if !label.trim().is_empty() => {
            OverlayItem::new(label.trim(), detail.trim())
        }
        _ => OverlayItem::new(line, ""),
    }
}

fn model_effort_overlay_for_target(
    catalog: &SelfAgentModelCatalog,
    configured: Option<&SelfAgentModelChoice>,
    target: ModelSelectionTarget,
) -> Option<TerminalOverlay> {
    let choice = match &target {
        ModelSelectionTarget::Configured => configured.cloned(),
        ModelSelectionTarget::Profile(name) => catalog
            .active
            .choice()
            .filter(|choice| choice.profile.as_deref() == Some(name))
            .cloned()
            .or_else(|| {
                catalog
                    .profiles
                    .iter()
                    .find(|choice| choice.profile.as_deref() == Some(name))
                    .cloned()
            }),
    }?;
    Some(TerminalOverlay::model_efforts(target, &choice))
}

fn load_model_effort_overlay(
    self_agent: &SelfAgentHost,
    target: ModelSelectionTarget,
) -> Result<Option<TerminalOverlay>, SelfAgentHostError> {
    let catalog = self_agent.model_catalog()?;
    let configured = match target {
        ModelSelectionTarget::Configured => Some(self_agent.configured_model_choice()?),
        ModelSelectionTarget::Profile(_) => None,
    };
    Ok(model_effort_overlay_for_target(
        &catalog,
        configured.as_ref(),
        target,
    ))
}

#[derive(Default)]
struct TerminalChatState {
    input: String,
    command_selected: usize,
    show_help: bool,
    transcript: Vec<TranscriptEntry>,
    pending: VecDeque<QueuedChatAction>,
    busy: Option<String>,
    scroll_offset: usize,
    overlay: Option<TerminalOverlay>,
    active_theme: theme::Theme,
    theme_before_overlay: Option<theme::Theme>,
}

impl TerminalChatState {
    fn view<'a>(&'a self, choices: &'a [AgentChoice], model_status: &'a str) -> ChatHomeView<'a> {
        ChatHomeView::new(
            &self.input,
            choices,
            self.command_selected,
            self.show_help,
            model_status,
            &self.transcript,
            self.busy.as_deref(),
        )
        .with_scroll(self.scroll_offset)
        .with_overlay(self.overlay.as_ref().map(|overlay| &overlay.view))
        .with_theme(self.active_theme)
    }

    fn clear_input(&mut self) {
        self.input.clear();
        self.command_selected = 0;
        self.show_help = false;
    }

    fn append_transcript(&mut self, entry: TranscriptEntry) {
        self.transcript.push(entry);
        self.scroll_offset = 0;
    }

    fn begin_assistant_transcript(&mut self) -> usize {
        let index = self.transcript.len();
        self.append_transcript(TranscriptEntry::assistant(String::new()));
        index
    }

    fn replace_assistant_snapshot(&mut self, index: usize, snapshot: &str) {
        let Some(entry) = self.transcript.get_mut(index) else {
            return;
        };
        if entry.role != interactive::TranscriptRole::Assistant {
            return;
        }
        let snapshot = interactive::clean_transcript_text(snapshot);
        let mut end = snapshot.len().min(orchester_modell::MAX_CONTENT_BYTES);
        while end > 0 && !snapshot.is_char_boundary(end) {
            end -= 1;
        }
        entry.text.clear();
        entry.text.push_str(&snapshot[..end]);
        self.scroll_offset = 0;
    }

    fn replace_transcript(&mut self, index: usize, entry: TranscriptEntry) {
        if let Some(current) = self.transcript.get_mut(index) {
            *current = entry;
            self.scroll_offset = 0;
        } else {
            self.append_transcript(entry);
        }
    }

    fn queue_action(&mut self, action: interactive::HomeAction) {
        match action {
            interactive::HomeAction::Submit(prompt) => {
                self.clear_input();
                self.append_transcript(TranscriptEntry::user(&prompt));
                self.pending
                    .push_back(QueuedChatAction::Turn(ModelTurnRequest::Prompt(prompt)));
            }
            interactive::HomeAction::Empty => {}
            other => {
                self.clear_input();
                self.append_transcript(TranscriptEntry::status(queued_command_label(&other)));
                self.pending.push_back(QueuedChatAction::Command(other));
            }
        }
    }

    fn queue_resume(&mut self, handle: String) {
        self.overlay = None;
        self.append_transcript(TranscriptEntry::status("Resuming selected run..."));
        self.pending
            .push_back(QueuedChatAction::Turn(ModelTurnRequest::Resume(handle)));
    }
}

enum ModelTurnResult {
    Completed(Box<Result<SelfAgentRunOutcome, SelfAgentHostError>>),
    Quit,
}

fn model_turn_redactor(
    result: Result<StreamingRedactor, SelfAgentHostError>,
) -> Result<StreamingRedactor, ModelTurnResult> {
    result.map_err(|error| ModelTurnResult::Completed(Box::new(Err(error))))
}

struct TtyEventSink {
    sender: watch::Sender<String>,
    redactor: Mutex<StreamingRedactor>,
    #[cfg(test)]
    publish_hook: Option<Arc<dyn Fn(&str) + Send + Sync>>,
}

impl TtyEventSink {
    fn publish(&self, snapshot: String) {
        #[cfg(test)]
        if let Some(hook) = self.publish_hook.as_ref() {
            hook(&snapshot);
        }
        let changed = self.sender.borrow().as_str() != snapshot;
        if changed {
            self.sender.send_replace(snapshot);
        }
    }
}

impl ModelEventSink for TtyEventSink {
    fn response_started(&self) {
        let Ok(mut redactor) = self.redactor.lock() else {
            return;
        };
        redactor.begin_response();
        self.publish(String::new());
        drop(redactor);
    }

    fn text_delta(&self, delta: &str) {
        let Ok(mut redactor) = self.redactor.lock() else {
            return;
        };
        let snapshot = redactor.push(delta).to_owned();
        self.publish(snapshot);
        drop(redactor);
    }

    fn response_completed(&self) {
        let Ok(mut redactor) = self.redactor.lock() else {
            return;
        };
        let snapshot = redactor.finish().to_owned();
        self.publish(snapshot);
        drop(redactor);
    }
}

fn busy_label(tick: usize) -> String {
    const FRAMES: [&str; 4] = ["Creating  ", "Creating .", "Creating ..", "Creating ..."];
    FRAMES[tick % FRAMES.len()].to_owned()
}

fn queued_command_label(action: &interactive::HomeAction) -> String {
    let command = match action {
        interactive::HomeAction::Workspace(command) => workspace_command_label(command),
        interactive::HomeAction::Plugins(PluginAction::List) => "/plugins".into(),
        interactive::HomeAction::Plugins(PluginAction::Status(name)) => {
            format!("/plugins status {name}")
        }
        interactive::HomeAction::Plugins(PluginAction::Install(name)) => {
            format!("/plugins install {name}")
        }
        interactive::HomeAction::Plugins(PluginAction::Remove(name)) => {
            format!("/plugins remove {name}")
        }
        interactive::HomeAction::PickAgent => "/agent".into(),
        interactive::HomeAction::LaunchAgent(name) => format!("/{name}"),
        interactive::HomeAction::Help => "/help".into(),
        interactive::HomeAction::Quit => "/quit".into(),
        interactive::HomeAction::Submit(_) | interactive::HomeAction::Empty => "command".into(),
    };
    interactive::clean_transcript_text(&format!("Queued: {command}"))
}

fn workspace_command_label(command: &WorkspaceCommand) -> String {
    match command {
        WorkspaceCommand::Status => "/status".into(),
        WorkspaceCommand::Config => "/config".into(),
        WorkspaceCommand::Permissions => "/permissions".into(),
        WorkspaceCommand::Resume => "/resume".into(),
        WorkspaceCommand::Model(ModelCommand::Show) => "/model".into(),
        WorkspaceCommand::Model(ModelCommand::SelectProfile(name)) => format!("/model {name}"),
        WorkspaceCommand::Model(ModelCommand::UseConfigured) => "/model configured".into(),
        WorkspaceCommand::Theme(_) => "/theme".into(),
        WorkspaceCommand::Credential(CredentialCommand::Login { provider }) => provider
            .as_deref()
            .map(|provider| format!("/login {provider}"))
            .unwrap_or_else(|| "/login".into()),
        WorkspaceCommand::Credential(CredentialCommand::Logout { provider }) => provider
            .as_deref()
            .map(|provider| format!("/logout {provider}"))
            .unwrap_or_else(|| "/logout".into()),
    }
}

async fn await_model_turn(
    chat: &mut interactive::ChatSession,
    self_agent: &mut SelfAgentHost,
    state: &mut TerminalChatState,
    choices: &[AgentChoice],
    model_status: &str,
    turn: ModelTurnRequest,
    assistant_index: usize,
) -> Result<ModelTurnResult, CliError> {
    let cancel = CancellationToken::new();
    let (sender, mut deltas) = watch::channel(String::new());
    let redactor = match model_turn_redactor(self_agent.streaming_redactor()) {
        Ok(redactor) => redactor,
        Err(result) => return Ok(result),
    };
    let sink: Arc<dyn ModelEventSink> = Arc::new(TtyEventSink {
        sender,
        redactor: Mutex::new(redactor),
        #[cfg(test)]
        publish_hook: None,
    });
    let request = async {
        match turn {
            ModelTurnRequest::Prompt(prompt) => {
                self_agent
                    .submit_with_events(prompt, cancel.clone(), Some(sink))
                    .await
            }
            ModelTurnRequest::Resume(handle) => {
                self_agent
                    .resume_with_events(&handle, cancel.clone(), Some(sink))
                    .await
            }
        }
    };
    tokio::pin!(request);
    let mut tick = 0usize;
    let mut cancel_requested = false;

    loop {
        tokio::select! {
            result = &mut request => {
                if cancel_requested {
                    return Ok(ModelTurnResult::Quit);
                }
                let delta = deltas.borrow_and_update().clone();
                state.replace_assistant_snapshot(assistant_index, &delta);
                return Ok(ModelTurnResult::Completed(Box::new(result)));
            }
            changed = deltas.changed() => {
                if changed.is_ok() {
                    let delta = deltas.borrow_and_update().clone();
                    state.replace_assistant_snapshot(assistant_index, &delta);
                    chat.present_view(state.view(choices, model_status))?;
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(120)) => {
                tick = tick.wrapping_add(1);
                state.busy = Some(busy_label(tick));
                if !cancel_requested {
                    while let Some(key) = chat.try_read_key()? {
                        let Some(action) = interactive::handle_chat_key_with_scroll(
                            key,
                            &mut state.input,
                            &mut state.command_selected,
                            &mut state.show_help,
                            &mut state.scroll_offset,
                            choices,
                        ) else {
                            continue;
                        };
                        if matches!(action, interactive::HomeAction::Quit) {
                            cancel.cancel();
                            cancel_requested = true;
                        } else {
                            state.queue_action(action);
                        }
                    }
                }
                chat.present_view(state.view(choices, model_status))?;
            }
        }
    }
}

async fn run_terminal_interactive(mut registry: Registry) -> Result<ExitCode, CliError> {
    let mut self_agent = self_agent_host()?;
    let mut chat = interactive::ChatSession::enter()?;
    let mut state = TerminalChatState {
        active_theme: theme::load_user_theme(),
        ..TerminalChatState::default()
    };

    loop {
        let choices = interactive::build_agent_choices(&registry);
        let model_status = self_agent
            .model_label()
            .unwrap_or_else(|_| "model unavailable".into());
        if state.overlay.is_some() {
            state.busy = None;
            chat.present_view(state.view(&choices, &model_status))?;
            let Some(key) = chat.read_key()? else {
                continue;
            };
            let overlay_input = state
                .overlay
                .as_mut()
                .and_then(|overlay| interactive::handle_overlay_key(key, &mut overlay.view));
            let Some(overlay_input) = overlay_input else {
                if let Some(theme) = state.overlay.as_ref().and_then(|overlay| {
                    overlay
                        .actions
                        .get(overlay.view.selected)
                        .and_then(|action| match action {
                            OverlayAction::Theme(theme) => Some(*theme),
                            _ => None,
                        })
                }) {
                    state.active_theme = theme;
                    if let Some(overlay) = state.overlay.as_mut() {
                        for (index, item) in overlay.view.items.iter_mut().enumerate() {
                            item.current = matches!(
                                overlay.actions.get(index),
                                Some(OverlayAction::Theme(candidate)) if *candidate == theme
                            );
                        }
                        overlay.refresh_theme_preview(theme);
                    }
                }
                continue;
            };
            match overlay_input {
                OverlayInput::Cancel => {
                    let parent = state
                        .overlay
                        .as_mut()
                        .and_then(|overlay| overlay.parent.take());
                    if let Some(parent) = parent {
                        state.overlay = Some(*parent);
                    } else {
                        if let Some(previous) = state.theme_before_overlay.take() {
                            state.active_theme = previous;
                        }
                        state.overlay = None;
                    }
                }
                OverlayInput::Confirm(index) => {
                    let action = state
                        .overlay
                        .as_ref()
                        .and_then(|overlay| overlay.actions.get(index))
                        .cloned()
                        .unwrap_or(OverlayAction::Close);
                    match action {
                        OverlayAction::Close => state.overlay = None,
                        OverlayAction::Inspect(_) => {
                            if let Some(overlay) = state.overlay.as_mut() {
                                overlay.inspect(index);
                            }
                        }
                        OverlayAction::Resume(handle) => state.queue_resume(handle),
                        OverlayAction::ModelConfigured | OverlayAction::ModelProfile(_) => {
                            let target = match action {
                                OverlayAction::ModelConfigured => ModelSelectionTarget::Configured,
                                OverlayAction::ModelProfile(name) => {
                                    ModelSelectionTarget::Profile(name)
                                }
                                _ => unreachable!(),
                            };
                            let parent = state.overlay.take();
                            state.overlay =
                                Some(match load_model_effort_overlay(&self_agent, target) {
                                    Ok(Some(child)) => match parent {
                                        Some(parent) => child.with_parent(parent),
                                        None => child,
                                    },
                                    Ok(None) => TerminalOverlay::error(
                                        "/model",
                                        &"selected model is unavailable",
                                    ),
                                    Err(error) => TerminalOverlay::error("/model", &error),
                                });
                        }
                        OverlayAction::ModelEffort { target, effort } => {
                            let mut rendered = Vec::new();
                            let result: Result<(), CliError> = (|| {
                                let choice = match target {
                                    ModelSelectionTarget::Configured => self_agent
                                        .select_configured_model_with_effort(effort.as_deref())?,
                                    ModelSelectionTarget::Profile(name) => self_agent
                                        .select_model_profile_with_effort(
                                            &name,
                                            effort.as_deref(),
                                        )?,
                                };
                                self_agent::render_model_selection(&mut rendered, &choice)?;
                                Ok(())
                            })();
                            state.overlay = Some(match result {
                                Ok(()) => TerminalOverlay::report(
                                    "/model",
                                    &String::from_utf8_lossy(&rendered),
                                ),
                                Err(error) => TerminalOverlay::error("/model", &error),
                            });
                        }
                        OverlayAction::Theme(selected) => match theme::persist_user_theme(selected)
                        {
                            Ok(()) => {
                                state.active_theme = selected;
                                state.theme_before_overlay = None;
                                state.overlay = Some(TerminalOverlay::report(
                                    "/theme",
                                    &format!("Theme saved\n{}", selected.label()),
                                ));
                            }
                            Err(error) => {
                                state.overlay = Some(TerminalOverlay::error("/theme", &error));
                            }
                        },
                    }
                }
            }
            continue;
        }
        if let Some(queued) = state.pending.pop_front() {
            match queued {
                QueuedChatAction::Turn(request) => {
                    let assistant_index = state.begin_assistant_transcript();
                    state.busy = Some(busy_label(0));
                    chat.present_view(state.view(&choices, &model_status))?;
                    match await_model_turn(
                        &mut chat,
                        &mut self_agent,
                        &mut state,
                        &choices,
                        &model_status,
                        request,
                        assistant_index,
                    )
                    .await?
                    {
                        ModelTurnResult::Quit => return Ok(ExitCode::SUCCESS),
                        ModelTurnResult::Completed(result) => match *result {
                            Ok(outcome) => {
                                let rendered = self_agent::render_outcome_transcript(&outcome)?;
                                if rendered.is_empty() {
                                    state.replace_transcript(
                                        assistant_index,
                                        TranscriptEntry::status("No response returned."),
                                    );
                                } else {
                                    state.replace_transcript(
                                        assistant_index,
                                        TranscriptEntry::assistant(rendered),
                                    );
                                }
                            }
                            Err(error) => state.replace_transcript(
                                assistant_index,
                                TranscriptEntry::error(error.to_string()),
                            ),
                        },
                    }
                    state.busy = None;
                }
                QueuedChatAction::Command(action) => match action {
                    interactive::HomeAction::Quit => return Ok(ExitCode::SUCCESS),
                    interactive::HomeAction::Submit(prompt) => {
                        state.queue_action(interactive::HomeAction::Submit(prompt));
                    }
                    interactive::HomeAction::Workspace(command) => {
                        let label = workspace_command_label(&command);
                        match command {
                            WorkspaceCommand::Status => {
                                state.overlay = Some(match self_agent.status() {
                                    Ok(status) => TerminalOverlay::inspection(
                                        workspace_overlay::status(&status),
                                    ),
                                    Err(error) => TerminalOverlay::error(&label, &error),
                                });
                            }
                            WorkspaceCommand::Config => {
                                state.overlay = Some(match self_agent.config_view() {
                                    Ok(view) => TerminalOverlay::inspection(
                                        workspace_overlay::config(&view),
                                    ),
                                    Err(error) => TerminalOverlay::error(&label, &error),
                                });
                            }
                            WorkspaceCommand::Permissions => {
                                state.overlay = Some(match self_agent.permissions() {
                                    Ok(permissions) => TerminalOverlay::inspection(
                                        workspace_overlay::permissions(&permissions),
                                    ),
                                    Err(error) => TerminalOverlay::error(&label, &error),
                                });
                            }
                            WorkspaceCommand::Resume => {
                                state.overlay = Some(match self_agent.resume_catalog() {
                                    Ok(catalog) => TerminalOverlay::resume(&catalog),
                                    Err(error) => TerminalOverlay::error(&label, &error),
                                });
                            }
                            WorkspaceCommand::Model(ModelCommand::Show) => {
                                state.overlay = Some(match self_agent.model_catalog() {
                                    Ok(catalog) => TerminalOverlay::models(&catalog),
                                    Err(error) => TerminalOverlay::error(&label, &error),
                                });
                            }
                            WorkspaceCommand::Model(ModelCommand::SelectProfile(name)) => {
                                state.overlay = Some(
                                    match load_model_effort_overlay(
                                        &self_agent,
                                        ModelSelectionTarget::Profile(name),
                                    ) {
                                        Ok(overlay) => overlay.unwrap_or_else(|| {
                                            TerminalOverlay::error(
                                                &label,
                                                &"selected model is unavailable",
                                            )
                                        }),
                                        Err(error) => TerminalOverlay::error(&label, &error),
                                    },
                                );
                            }
                            WorkspaceCommand::Model(ModelCommand::UseConfigured) => {
                                state.overlay = Some(
                                    match load_model_effort_overlay(
                                        &self_agent,
                                        ModelSelectionTarget::Configured,
                                    ) {
                                        Ok(overlay) => overlay.unwrap_or_else(|| {
                                            TerminalOverlay::error(
                                                &label,
                                                &"configured model is unavailable",
                                            )
                                        }),
                                        Err(error) => TerminalOverlay::error(&label, &error),
                                    },
                                );
                            }
                            WorkspaceCommand::Theme(ThemeCommand::Show) => {
                                state.theme_before_overlay = Some(state.active_theme);
                                state.overlay = Some(TerminalOverlay::themes(state.active_theme));
                            }
                            WorkspaceCommand::Theme(ThemeCommand::Select(name)) => {
                                let selected = theme::Theme::from_stored_name(&name);
                                match theme::persist_user_theme(selected) {
                                    Ok(()) => {
                                        state.active_theme = selected;
                                        state.theme_before_overlay = None;
                                        state.overlay = Some(TerminalOverlay::report(
                                            "/theme",
                                            &format!("Theme saved\n{}", selected.label()),
                                        ));
                                    }
                                    Err(error) => {
                                        state.overlay =
                                            Some(TerminalOverlay::error("/theme", &error));
                                    }
                                }
                            }
                            WorkspaceCommand::Credential(command) => {
                                drop(chat);
                                let mut rendered = Vec::new();
                                let result = render_workspace_command_to(
                                    &mut self_agent,
                                    WorkspaceCommand::Credential(command),
                                    &mut rendered,
                                );
                                chat = interactive::ChatSession::enter()?;
                                state.overlay = Some(match result {
                                    Ok(()) => TerminalOverlay::report(
                                        &label,
                                        &String::from_utf8_lossy(&rendered),
                                    ),
                                    Err(error) => TerminalOverlay::error(&label, &error),
                                });
                            }
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
                                    Err(error) => state.append_transcript(TranscriptEntry::error(
                                        error.to_string(),
                                    )),
                                }
                            }
                        }
                    }
                    interactive::HomeAction::LaunchAgent(name) => {
                        let Some(agent) = choices.iter().find(|choice| choice.name == name) else {
                            state.append_transcript(TranscriptEntry::error(format!(
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
                                Err(error) => state
                                    .append_transcript(TranscriptEntry::error(error.to_string())),
                            }
                        }
                    }
                    interactive::HomeAction::Plugins(action) => {
                        if matches!(&action, PluginAction::List) {
                            state.overlay = Some(TerminalOverlay::inspection(
                                workspace_overlay::plugins(&registry.plugins()),
                            ));
                            continue;
                        }
                        let result = run_plugin_command_to_transcript(
                            &registry,
                            plugin_command(action),
                            &orchester_home(),
                        );
                        match result {
                            Ok((outcome, _, error)) if outcome.failed() => {
                                let error = if error.is_empty() {
                                    "plugin command failed".into()
                                } else {
                                    error
                                };
                                state.overlay = Some(TerminalOverlay::report(
                                    "/plugins",
                                    &format!("Error\n{error}"),
                                ));
                            }
                            Ok((_, output, _)) => {
                                state.overlay = Some(TerminalOverlay::report("/plugins", &output));
                                registry = discover_registry()?;
                            }
                            Err(error) => {
                                state.overlay = Some(TerminalOverlay::error("/plugins", &error));
                            }
                        }
                    }
                    interactive::HomeAction::Help => state.show_help = true,
                },
            }
            continue;
        }

        state.busy = None;
        chat.present_view(state.view(&choices, &model_status))?;
        let Some(key) = chat.read_key()? else {
            continue;
        };
        if let Some(action) = interactive::handle_chat_key_with_scroll(
            key,
            &mut state.input,
            &mut state.command_selected,
            &mut state.show_help,
            &mut state.scroll_offset,
            &choices,
        ) {
            if matches!(action, interactive::HomeAction::Quit) {
                return Ok(ExitCode::SUCCESS);
            }
            state.queue_action(action);
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
    let mut had_error = false;
    let mut initial_agent = loop {
        let Some(line) = interactive::read_startup_line(&mut input)? else {
            return Ok(if had_error {
                ExitCode::FAILURE
            } else if received_input {
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
                match self_agent.submit(prompt, CancellationToken::new()).await {
                    Ok(outcome) => {
                        let mut out = io::stdout().lock();
                        self_agent::render_outcome(&mut out, &outcome)?;
                        interactive::render_line_continue_prompt(&mut out)?;
                    }
                    Err(error) => {
                        had_error = true;
                        let mut err = io::stderr().lock();
                        writeln!(err, "orchester: {error}")?;
                        drop(err);
                        let mut out = io::stdout().lock();
                        interactive::render_line_continue_prompt(&mut out)?;
                    }
                }
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

fn run_plugin_command_to_transcript(
    registry: &Registry,
    command: PluginCommand,
    orchester_home: &std::path::Path,
) -> io::Result<(plugin::PluginOutcome, String, String)> {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = plugin::run_to(registry, command, false, orchester_home, &mut out, &mut err)?;
    let output = interactive::clean_transcript_text(&String::from_utf8_lossy(&out));
    let error = interactive::clean_transcript_text(&String::from_utf8_lossy(&err));
    Ok((outcome, output, error))
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
    let mut out = io::stdout().lock();
    render_workspace_command_to(self_agent, command, &mut out)
}

fn render_workspace_command_to<W: Write>(
    self_agent: &mut SelfAgentHost,
    command: WorkspaceCommand,
    out: &mut W,
) -> Result<(), CliError> {
    match command {
        WorkspaceCommand::Status => {
            let status = self_agent.status()?;
            self_agent::render_status(out, &status)?;
        }
        WorkspaceCommand::Config => {
            let view = self_agent.config_view()?;
            self_agent::render_config(out, &view)?;
        }
        WorkspaceCommand::Permissions => {
            let permissions = self_agent.permissions()?;
            self_agent::render_permissions(out, &permissions)?;
        }
        WorkspaceCommand::Resume => {
            let catalog = self_agent.resume_catalog()?;
            self_agent::render_resume(out, &catalog)?;
        }
        WorkspaceCommand::Model(ModelCommand::Show) => {
            let models = self_agent.model_catalog()?;
            self_agent::render_models(out, &models)?;
        }
        WorkspaceCommand::Model(ModelCommand::SelectProfile(name)) => {
            let selected = self_agent.select_model_profile(&name)?;
            self_agent::render_model_selection(out, &selected)?;
        }
        WorkspaceCommand::Model(ModelCommand::UseConfigured) => {
            let selected = self_agent.select_configured_model()?;
            self_agent::render_model_selection(out, &selected)?;
        }
        WorkspaceCommand::Theme(command) => render_theme_command_to(out, command)?,
        WorkspaceCommand::Credential(CredentialCommand::Login { provider }) => {
            let target = self_agent.credential_target(provider.as_deref())?;
            self_agent::render_credential_target(out, &target)?;
            // The prompt writes to the same terminal, so the stdout lock is
            // released before asking and retaken to confirm.
            let Some(secret) = interactive::prompt_secret("API key")? else {
                writeln!(out, "cancelled; nothing was stored")?;
                return Ok(());
            };
            let (update, wiring, config_path) = self_agent.store_credential(&target, secret)?;
            self_agent::render_credential_stored(out, &update, &wiring, &config_path)?;
        }
        WorkspaceCommand::Credential(CredentialCommand::Logout { provider }) => {
            let target = self_agent.credential_target(provider.as_deref())?;
            let removed = self_agent.clear_credential(&target.provider)?;
            self_agent::render_credential_cleared(out, &target.provider, removed)?;
        }
    }
    Ok(())
}

fn render_theme_command_to<W: Write>(out: &mut W, command: ThemeCommand) -> Result<(), CliError> {
    match command {
        ThemeCommand::Show => {
            writeln!(out, "Terminal themes")?;
            let current = theme::load_user_theme();
            for candidate in theme::Theme::all() {
                writeln!(
                    out,
                    "{} {} ({})",
                    if candidate == current { ">" } else { " " },
                    candidate.label(),
                    candidate.name()
                )?;
            }
            writeln!(out, "Use /theme <name> to save a theme.")?;
        }
        ThemeCommand::Select(name) => {
            let selected = theme::Theme::from_stored_name(&name);
            theme::persist_user_theme(selected)?;
            writeln!(
                out,
                "Theme saved: {} ({})",
                selected.label(),
                selected.name()
            )?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_laufzeit::harness::service::{
        SelfAgentResumeAvailability, SelfAgentResumeCatalog, SelfAgentResumeEntry,
        SelfAgentResumeStep,
    };
    use secrecy::SecretString;
    use std::path::Path;
    use std::sync::{Condvar, Mutex as StdMutex};

    #[test]
    fn queued_prompt_is_recorded_before_a_busy_turn_starts() {
        let mut state = TerminalChatState {
            input: "next question".into(),
            ..TerminalChatState::default()
        };

        state.queue_action(interactive::HomeAction::Submit("next question".into()));

        assert!(state.input.is_empty());
        assert_eq!(state.transcript.len(), 1);
        assert_eq!(state.transcript[0].text, "next question");
        assert!(matches!(
            state.pending.front(),
            Some(QueuedChatAction::Turn(ModelTurnRequest::Prompt(prompt)))
                if prompt == "next question"
        ));
    }

    #[test]
    fn busy_animation_cycles_without_growing_the_label() {
        assert_eq!(busy_label(0), "Creating  ");
        assert_eq!(busy_label(1), "Creating .");
        assert_eq!(busy_label(4), "Creating  ");
        assert!(busy_label(99).len() <= "Creating ...".len());
    }

    #[test]
    fn command_report_overlay_is_bounded_to_selectable_sanitized_rows() {
        let mut overlay = TerminalOverlay::report(
            "/status",
            "\x1b[1mSelf-agent status\x1b[0m\nmodel: gpt-test\nstate: ready",
        );

        assert_eq!(overlay.view.title, "/status result");
        assert_eq!(overlay.view.items.len(), 3);
        assert_eq!(overlay.view.items[0].label, "Self-agent status");
        assert_eq!(overlay.view.items[1].label, "model");
        assert_eq!(overlay.view.items[1].detail, "gpt-test");
        assert!(!overlay.view.items[0].label.contains('\x1b'));
        assert_eq!(overlay.actions.len(), overlay.view.items.len());
        assert!(overlay
            .actions
            .iter()
            .all(|action| matches!(action, OverlayAction::Inspect(_))));
        assert!(overlay.view.footer.contains("Enter inspect"));
        assert!(overlay.inspect(1));
        assert_eq!(overlay.view.details, vec!["model: gpt-test"]);
    }

    #[test]
    fn resume_overlay_assigns_resume_actions_only_to_ready_rows() {
        let catalog = SelfAgentResumeCatalog {
            database_present: true,
            truncated: false,
            entries: vec![
                SelfAgentResumeEntry {
                    handle: "r-ready".into(),
                    availability: SelfAgentResumeAvailability::Ready,
                    step: SelfAgentResumeStep::StartNextStep,
                    latest: true,
                },
                SelfAgentResumeEntry {
                    handle: "r-unsupported".into(),
                    availability: SelfAgentResumeAvailability::Unsupported,
                    step: SelfAgentResumeStep::StartStep,
                    latest: false,
                },
                SelfAgentResumeEntry {
                    handle: "r-approval".into(),
                    availability: SelfAgentResumeAvailability::ApprovalRequired,
                    step: SelfAgentResumeStep::AwaitApproval,
                    latest: false,
                },
                SelfAgentResumeEntry {
                    handle: "r-reconcile".into(),
                    availability: SelfAgentResumeAvailability::ReconciliationRequired,
                    step: SelfAgentResumeStep::ReconcileToolOutcome,
                    latest: false,
                },
            ],
        };

        let overlay = TerminalOverlay::resume(&catalog);

        assert_eq!(overlay.actions.len(), overlay.view.items.len());
        assert!(matches!(
            overlay.actions.first(),
            Some(OverlayAction::Resume(handle)) if handle == "r-ready"
        ));
        assert!(overlay
            .actions
            .iter()
            .skip(1)
            .all(|action| matches!(action, OverlayAction::Inspect(_))));
    }

    #[test]
    fn queued_resume_closes_the_overlay_without_adding_a_user_prompt() {
        let catalog = SelfAgentResumeCatalog {
            database_present: true,
            truncated: false,
            entries: vec![SelfAgentResumeEntry {
                handle: "r-ready".into(),
                availability: SelfAgentResumeAvailability::Ready,
                step: SelfAgentResumeStep::StartNextStep,
                latest: true,
            }],
        };
        let mut state = TerminalChatState {
            overlay: Some(TerminalOverlay::resume(&catalog)),
            ..TerminalChatState::default()
        };

        state.queue_resume("r-ready".into());

        assert!(state.overlay.is_none());
        assert_eq!(state.transcript.len(), 1);
        assert_eq!(
            state.transcript[0].role,
            interactive::TranscriptRole::Status
        );
        assert_eq!(state.transcript[0].text, "Resuming selected run...");
        assert!(matches!(
            state.pending.front(),
            Some(QueuedChatAction::Turn(ModelTurnRequest::Resume(handle)))
                if handle == "r-ready"
        ));
    }

    #[test]
    fn theme_overlay_marks_exactly_the_persisted_theme_as_current() {
        let overlay = TerminalOverlay::themes(theme::Theme::LightColorblind);

        assert_eq!(
            overlay
                .view
                .items
                .iter()
                .filter(|item| item.current)
                .map(|item| item.detail.as_str())
                .collect::<Vec<_>>(),
            vec!["light-colorblind"]
        );
        assert!(matches!(
            overlay.actions.get(4),
            Some(OverlayAction::Theme(theme::Theme::LightColorblind))
        ));
        assert_eq!(overlay.view.selected, 4);
    }

    #[test]
    fn theme_overlay_contains_a_preview_for_the_selected_palette() {
        let overlay = TerminalOverlay::themes(theme::Theme::DarkColorblind);

        assert!(overlay
            .view
            .details
            .iter()
            .any(|line| line.contains("Preview: Dark mode (colorblind-friendly)")));
        assert!(overlay
            .view
            .details
            .iter()
            .any(|line| line.contains("assistant output")));
    }

    #[test]
    fn theme_preview_refreshes_without_rebuilding_the_option_list() {
        let mut overlay = TerminalOverlay::themes(theme::Theme::Default);
        let item_count = overlay.view.items.len();

        overlay.refresh_theme_preview(theme::Theme::Light);

        assert_eq!(overlay.view.items.len(), item_count);
        assert!(overlay
            .view
            .details
            .first()
            .is_some_and(|line| line.contains("Preview: Light")));
    }

    #[test]
    fn model_overlay_prompts_for_effort_before_applying_a_choice() {
        let catalog = SelfAgentModelCatalog {
            active: SelfAgentActiveModel::Configured(
                orchester_laufzeit::harness::service::SelfAgentModelChoice {
                    profile: None,
                    provider: "openai".into(),
                    provider_name: "OpenAI".into(),
                    model: "gpt-test".into(),
                    reasoning_effort: Some("high".into()),
                    plan_reasoning_effort: None,
                    service_tier: None,
                },
            ),
            profiles: Vec::new(),
        };

        let overlay = TerminalOverlay::models(&catalog);

        assert!(overlay.view.footer.contains("choose effort"));
    }

    #[test]
    fn model_effort_overlay_marks_the_catalogued_effort_and_keeps_target() {
        let choice = SelfAgentModelChoice {
            profile: Some("balanced".into()),
            provider: "openai".into(),
            provider_name: "OpenAI".into(),
            model: "gpt-test".into(),
            reasoning_effort: Some("high".into()),
            plan_reasoning_effort: None,
            service_tier: None,
        };

        let overlay = TerminalOverlay::model_efforts(
            ModelSelectionTarget::Profile("balanced".into()),
            &choice,
        );

        assert_eq!(
            overlay
                .view
                .items
                .iter()
                .filter(|item| item.current)
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["high"]
        );
        assert_eq!(overlay.view.items.len(), 6);
        assert!(matches!(
            overlay.actions.get(3),
            Some(OverlayAction::ModelEffort {
                target: ModelSelectionTarget::Profile(name),
                effort: Some(effort),
            }) if name == "balanced" && effort == "high"
        ));
        assert!(overlay.view.footer.contains("Esc back"));
    }

    #[test]
    fn model_effort_overlay_uses_provider_default_when_no_effort_is_configured() {
        let choice = SelfAgentModelChoice {
            profile: None,
            provider: "openai".into(),
            provider_name: "OpenAI".into(),
            model: "gpt-test".into(),
            reasoning_effort: None,
            plan_reasoning_effort: None,
            service_tier: None,
        };

        let overlay = TerminalOverlay::model_efforts(ModelSelectionTarget::Configured, &choice);

        assert!(overlay.view.items[0].current);
        assert!(matches!(
            overlay.actions.first(),
            Some(OverlayAction::ModelEffort {
                target: ModelSelectionTarget::Configured,
                effort: None,
            })
        ));
    }

    #[test]
    fn configured_effort_picker_does_not_reuse_the_active_named_profile() {
        let configured = SelfAgentModelChoice {
            profile: None,
            provider: "openai".into(),
            provider_name: "OpenAI".into(),
            model: "gpt-default".into(),
            reasoning_effort: Some("medium".into()),
            plan_reasoning_effort: None,
            service_tier: None,
        };
        let active_profile = SelfAgentModelChoice {
            profile: Some("fast".into()),
            model: "gpt-fast".into(),
            reasoning_effort: Some("low".into()),
            ..configured.clone()
        };
        let catalog = SelfAgentModelCatalog {
            active: SelfAgentActiveModel::Configured(active_profile.clone()),
            profiles: vec![active_profile],
        };

        let overlay = model_effort_overlay_for_target(
            &catalog,
            Some(&configured),
            ModelSelectionTarget::Configured,
        )
        .expect("configured picker");

        assert!(overlay.view.description.contains("gpt-default"));
        assert!(!overlay.view.description.contains("gpt-fast"));
        assert!(overlay.view.items[2].current, "medium must be current");
    }

    #[test]
    fn active_profile_effort_picker_uses_the_session_override() {
        let stored = SelfAgentModelChoice {
            profile: Some("fast".into()),
            provider: "openai".into(),
            provider_name: "OpenAI".into(),
            model: "gpt-fast".into(),
            reasoning_effort: Some("low".into()),
            plan_reasoning_effort: None,
            service_tier: None,
        };
        let mut active = stored.clone();
        active.reasoning_effort = Some("ultra".into());
        let catalog = SelfAgentModelCatalog {
            active: SelfAgentActiveModel::Configured(active),
            profiles: vec![stored],
        };

        let overlay = model_effort_overlay_for_target(
            &catalog,
            None,
            ModelSelectionTarget::Profile("fast".into()),
        )
        .expect("profile picker");

        assert!(overlay.view.items[5].current, "ultra must be current");
        assert!(!overlay.view.items[1].current, "stored low must not win");
    }

    #[test]
    fn model_effort_overlay_keeps_a_parent_for_escape_back_navigation() {
        let choice = SelfAgentModelChoice {
            profile: Some("balanced".into()),
            provider: "openai".into(),
            provider_name: "OpenAI".into(),
            model: "gpt-test".into(),
            reasoning_effort: Some("medium".into()),
            plan_reasoning_effort: None,
            service_tier: None,
        };
        let parent = TerminalOverlay::models(&SelfAgentModelCatalog {
            active: SelfAgentActiveModel::Configured(choice.clone()),
            profiles: vec![choice.clone()],
        });
        let child = TerminalOverlay::model_efforts(
            ModelSelectionTarget::Profile("balanced".into()),
            &choice,
        )
        .with_parent(parent);

        assert_eq!(
            child
                .parent
                .as_ref()
                .map(|parent| parent.view.title.as_str()),
            Some("Select model")
        );
    }

    #[test]
    fn queued_workspace_command_is_recorded_with_a_sanitized_marker() {
        let mut state = TerminalChatState {
            input: "/status\x1b[31m".into(),
            ..TerminalChatState::default()
        };

        state.queue_action(interactive::HomeAction::Workspace(WorkspaceCommand::Status));

        assert_eq!(state.transcript.len(), 1);
        assert_eq!(state.transcript[0].text, "Queued: /status");
        assert!(matches!(
            state.pending.front(),
            Some(QueuedChatAction::Command(
                interactive::HomeAction::Workspace(WorkspaceCommand::Status)
            ))
        ));
    }

    #[test]
    fn streamed_snapshots_replace_one_assistant_transcript_entry() {
        let mut state = TerminalChatState::default();

        let assistant = state.begin_assistant_transcript();
        state.replace_assistant_snapshot(assistant, "hello ");
        state.replace_assistant_snapshot(assistant, "hello world");

        assert_eq!(state.transcript.len(), 1);
        assert_eq!(
            state.transcript[assistant].role,
            interactive::TranscriptRole::Assistant
        );
        assert_eq!(state.transcript[assistant].text, "hello world");
    }

    #[test]
    fn tty_event_sink_redacts_a_secret_split_across_provider_deltas() {
        let (sender, mut snapshots) = watch::channel(String::new());
        let sink = TtyEventSink {
            sender,
            redactor: Mutex::new(StreamingRedactor::new(vec![SecretString::new(
                "stream-secret-canary".to_owned().into_boxed_str(),
            )])),
            publish_hook: None,
        };

        sink.text_delta(&format!("safe {} stream-secret-", "x".repeat(80)));
        sink.text_delta("canary done");
        sink.response_completed();

        let latest = snapshots.borrow_and_update().clone();
        assert!(!latest.contains("stream-secret-canary"));
        assert!(latest.contains("[REDACTED]"));
    }

    #[test]
    fn tty_event_sink_starts_each_model_response_with_fresh_state() {
        let (sender, mut snapshots) = watch::channel(String::new());
        let sink = TtyEventSink {
            sender,
            redactor: Mutex::new(StreamingRedactor::new(Vec::new())),
            publish_hook: None,
        };

        sink.response_started();
        sink.text_delta(&format!("first response {}", "a".repeat(300)));
        sink.response_completed();
        sink.response_started();
        sink.text_delta(&format!("second response {}", "b".repeat(300)));
        sink.response_completed();

        let latest = snapshots.borrow_and_update().clone();
        assert!(latest.contains("second response"));
        assert!(!latest.contains("first response"));
    }

    #[test]
    fn tty_event_sink_keeps_only_the_latest_stream_snapshot() {
        let (sender, mut snapshots) = watch::channel(String::new());
        let sink = TtyEventSink {
            sender,
            redactor: Mutex::new(StreamingRedactor::new(Vec::new())),
            publish_hook: None,
        };

        sink.response_started();
        for index in 0..64 {
            sink.text_delta(&format!("chunk-{index:04} {}", "x".repeat(256)));
        }
        sink.response_completed();

        assert!(snapshots.has_changed().expect("open snapshot channel"));
        let latest = snapshots.borrow_and_update().clone();
        assert!(latest.contains("chunk-0000"));
        assert!(!snapshots.has_changed().expect("open snapshot channel"));
    }

    #[test]
    fn tty_event_sink_never_publishes_an_older_concurrent_snapshot_last() {
        #[derive(Default)]
        struct PublishOrder {
            first_waiting: bool,
            second_published: bool,
        }

        let (sender, snapshots) = watch::channel(String::new());
        let order = Arc::new((StdMutex::new(PublishOrder::default()), Condvar::new()));
        let hook_order = Arc::clone(&order);
        let sink = Arc::new(TtyEventSink {
            sender,
            redactor: Mutex::new(StreamingRedactor::new(Vec::new())),
            publish_hook: Some(Arc::new(move |snapshot| {
                let (state, changed) = &*hook_order;
                let mut state = state.lock().expect("publish order lock");
                if snapshot.starts_with("first") && !snapshot.contains("second") {
                    state.first_waiting = true;
                    changed.notify_all();
                    while !state.second_published {
                        let (next, timeout) = changed
                            .wait_timeout(state, Duration::from_secs(1))
                            .expect("publish order wait");
                        state = next;
                        if timeout.timed_out() {
                            break;
                        }
                    }
                } else if snapshot.contains("second") {
                    state.second_published = true;
                    changed.notify_all();
                }
            })),
        });

        let first_sink = Arc::clone(&sink);
        let first =
            std::thread::spawn(move || first_sink.text_delta(&format!("first{}", "a".repeat(128))));
        {
            let (state, changed) = &*order;
            let state = state.lock().expect("publish order lock");
            let (state, _) = changed
                .wait_timeout_while(state, Duration::from_secs(2), |state| !state.first_waiting)
                .expect("first publish wait");
            assert!(state.first_waiting, "first snapshot never reached publish");
        }

        let second_sink = Arc::clone(&sink);
        let second = std::thread::spawn(move || {
            second_sink.text_delta(&format!("second{}", "b".repeat(512)))
        });
        first.join().expect("first delta thread");
        second.join().expect("second delta thread");

        assert!(
            order.0.lock().expect("publish order lock").second_published,
            "newer snapshot never reached publish"
        );
        assert!(snapshots.borrow().contains("second"));
    }

    #[test]
    fn streaming_redactor_initialization_error_stays_inside_the_chat_turn() {
        let result = model_turn_redactor(Err(SelfAgentHostError::Initialization));

        let Err(ModelTurnResult::Completed(error)) = result else {
            panic!("initialization error escaped the chat turn");
        };
        assert!(matches!(*error, Err(SelfAgentHostError::Initialization)));
    }

    #[test]
    fn plugin_command_output_is_captured_for_the_chat_transcript() {
        let (outcome, output, error) = run_plugin_command_to_transcript(
            &Registry::new(),
            PluginCommand::List,
            Path::new("unused"),
        )
        .expect("capture plugin output");

        assert_eq!(outcome, plugin::PluginOutcome::Succeeded);
        assert_eq!(output, "no agent plugins installed");
        assert!(error.is_empty());
    }
}
