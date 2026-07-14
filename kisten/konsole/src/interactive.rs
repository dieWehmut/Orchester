use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, Write};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

mod screen;

use crate::avatar;
use crossterm::event::{
    self, Event as TerminalEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::terminal;
use orchester_protokoll::{Capability, TaskKind};
use orchester_vertrag::{AdapterAvailability, AvailabilityStatus};
use orchester_verzeichnis::Registry;
use screen::{FramePresenter, TerminalSession};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const ORANGE: &str = "\x1b[38;5;208m";
const RESET: &str = "\x1b[0m";
const COMPACT_PALETTE_ROWS: usize = 6;
const PALETTE_ROWS: usize = 8;
const PICKER_PANEL_ROWS: usize = 7;

const PROMPT_SUGGESTIONS: [&str; 6] = [
    "Summarize recent commits",
    "Find and fix a bug in @filename",
    "Explain how <filepath> works",
    "Add tests for the current change",
    "Review my working tree",
    "Trace the cause of a failing command",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentChoice {
    pub name: String,
    pub kinds: String,
    pub supports_resume: bool,
    pub status: AvailabilityStatus,
    pub detail: String,
    pub native_command: Option<String>,
}

impl AgentChoice {
    pub fn is_available(&self) -> bool {
        self.status != AvailabilityStatus::Missing
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptAction {
    Run(String),
    PickAgent,
    LaunchAgent(String),
    ListAgents,
    Help,
    Quit,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeAction {
    Submit(String),
    PickAgent,
    LaunchAgent(String),
    Help,
    Quit,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandAction {
    PickAgent,
    ListAgents,
    Help,
    Quit,
    LaunchAgent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandItem {
    name: String,
    description: String,
    action: CommandAction,
    agent: Option<String>,
}

#[derive(Clone, Copy)]
struct PickerView<'a> {
    width: usize,
    height: usize,
    choices: &'a [AgentChoice],
    selected: usize,
    command: &'a str,
    command_selected: usize,
    message: &'a str,
}

pub fn run_home_tui(choices: &[AgentChoice]) -> io::Result<HomeAction> {
    let _session = TerminalSession::enter()?;
    let mut out = io::stdout();
    let mut presenter = FramePresenter::default();
    let mut input = String::new();
    let mut command_selected = 0usize;
    let mut show_help = false;

    loop {
        let (cols, rows) = terminal::size().unwrap_or((100, 30));
        present_chat_home_in_viewport(
            &mut presenter,
            &mut out,
            viewport_content_width(cols),
            (rows as usize).max(1),
            &input,
            choices,
            command_selected,
            show_help,
        )?;

        let TerminalEvent::Key(key) = event::read()? else {
            continue;
        };
        if !is_press(&key) {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(HomeAction::Quit);
        }

        match key.code {
            KeyCode::Enter => {
                let action = parse_home_action_selected(&input, choices, command_selected);
                if matches!(action, HomeAction::Help) {
                    input.clear();
                    command_selected = 0;
                    show_help = true;
                    continue;
                }
                if !matches!(action, HomeAction::Empty) {
                    return Ok(action);
                }
            }
            KeyCode::Esc => {
                if show_help {
                    show_help = false;
                    continue;
                }
                if input.is_empty() {
                    return Ok(HomeAction::Quit);
                }
                input.clear();
                command_selected = 0;
            }
            KeyCode::Backspace => {
                input.pop();
                command_selected = 0;
                show_help = false;
            }
            KeyCode::Up if input.starts_with('/') => {
                let matches = matching_commands(&input, choices);
                command_selected = wrapped_selection(
                    command_selected,
                    matches.len(),
                    SelectionDirection::Previous,
                );
            }
            KeyCode::Down if input.starts_with('/') => {
                let matches = matching_commands(&input, choices);
                command_selected =
                    wrapped_selection(command_selected, matches.len(), SelectionDirection::Next);
            }
            KeyCode::Char(ch) => {
                input.push(ch);
                command_selected = 0;
                show_help = false;
            }
            _ => {}
        }
    }
}

fn is_press(key: &KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionDirection {
    Previous,
    Next,
}

fn wrapped_selection(selected: usize, item_count: usize, direction: SelectionDirection) -> usize {
    if item_count == 0 {
        return 0;
    }

    let selected = selected.min(item_count - 1);
    match direction {
        SelectionDirection::Previous if selected == 0 => item_count - 1,
        SelectionDirection::Previous => selected - 1,
        SelectionDirection::Next => (selected + 1) % item_count,
    }
}

fn selection_window_start(selected: usize, item_count: usize, visible_rows: usize) -> usize {
    if item_count <= visible_rows || visible_rows == 0 {
        return 0;
    }
    selected
        .min(item_count - 1)
        .saturating_add(1)
        .saturating_sub(visible_rows)
}

pub fn build_agent_choices(registry: &Registry) -> Vec<AgentChoice> {
    let caps: BTreeMap<_, _> = registry
        .list()
        .into_iter()
        .map(|cap| (cap.name.clone(), cap))
        .collect();
    let availability: HashMap<_, _> = registry
        .availability()
        .into_iter()
        .map(|check| (check.name.clone(), check))
        .collect();

    let mut choices = caps
        .values()
        .map(|cap| {
            let check = availability
                .get(&cap.name)
                .cloned()
                .unwrap_or_else(|| AdapterAvailability::unknown(&cap.name, "not checked"));
            AgentChoice {
                name: cap.name.clone(),
                kinds: render_kinds(cap),
                supports_resume: cap.supports_resume,
                status: check.status,
                detail: check.detail,
                native_command: registry.native_command(&cap.name),
            }
        })
        .collect::<Vec<_>>();

    choices.sort_by(|a, b| {
        status_rank(a.status)
            .cmp(&status_rank(b.status))
            .then_with(|| native_rank(a).cmp(&native_rank(b)))
            .then_with(|| agent_rank(&a.name).cmp(&agent_rank(&b.name)))
            .then_with(|| a.name.cmp(&b.name))
    });
    choices
}

pub fn select_agent_tui(
    choices: &[AgentChoice],
    default_agent: Option<&str>,
) -> io::Result<Option<AgentChoice>> {
    let selectable = selectable_agents(choices);
    if selectable.is_empty() {
        let mut out = io::stdout().lock();
        render_no_runnable_agents(&mut out)?;
        return Ok(None);
    }

    let _session = TerminalSession::enter()?;
    let mut out = io::stdout();
    let mut presenter = FramePresenter::default();
    let mut selected = default_index(&selectable, default_agent);
    let mut command = String::new();
    let mut command_selected = 0usize;
    let mut message = String::new();

    loop {
        let (cols, rows) = terminal::size().unwrap_or((100, 30));
        present_home(
            &mut presenter,
            &mut out,
            PickerView {
                width: viewport_content_width(cols),
                height: usize::from(rows).max(1),
                choices,
                selected,
                command: &command,
                command_selected,
                message: &message,
            },
        )?;

        let TerminalEvent::Key(key) = event::read()? else {
            continue;
        };
        if !is_press(&key) {
            continue;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(None);
        }

        if command.starts_with('/') {
            let matches = matching_commands(&command, choices);
            match key.code {
                KeyCode::Esc => {
                    command.clear();
                    command_selected = 0;
                    message.clear();
                }
                KeyCode::Backspace => {
                    command.pop();
                    command_selected = 0;
                    message.clear();
                }
                KeyCode::Up => {
                    command_selected = wrapped_selection(
                        command_selected,
                        matches.len(),
                        SelectionDirection::Previous,
                    );
                }
                KeyCode::Down => {
                    command_selected = wrapped_selection(
                        command_selected,
                        matches.len(),
                        SelectionDirection::Next,
                    );
                }
                KeyCode::Enter => {
                    let action = command_action(&command, matches.get(command_selected));
                    match action {
                        PromptAction::Quit => {
                            return Ok(None);
                        }
                        PromptAction::LaunchAgent(name) => {
                            if let Some(agent) = choices.iter().find(|choice| choice.name == name) {
                                return Ok(Some(agent.clone()));
                            }
                            message = format!("Agent not available: {name}");
                        }
                        PromptAction::PickAgent
                        | PromptAction::ListAgents
                        | PromptAction::Empty => {
                            command.clear();
                            command_selected = 0;
                            message.clear();
                        }
                        PromptAction::Help => {
                            message = "Use Up/Down to choose an agent, Enter to launch it, or type / to search commands.".into();
                            command.clear();
                            command_selected = 0;
                        }
                        PromptAction::Run(_) => {}
                    }
                }
                KeyCode::Char(c) => {
                    command.push(c);
                    command_selected = 0;
                    message.clear();
                }
                _ => {}
            }
            continue;
        }

        match key.code {
            KeyCode::Esc => {
                return Ok(None);
            }
            KeyCode::Char('/') => {
                command.push('/');
                command_selected = 0;
                message.clear();
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                return Ok(None);
            }
            KeyCode::Up => {
                selected =
                    wrapped_selection(selected, selectable.len(), SelectionDirection::Previous);
                message.clear();
            }
            KeyCode::Down => {
                selected = wrapped_selection(selected, selectable.len(), SelectionDirection::Next);
                message.clear();
            }
            KeyCode::Enter => {
                return Ok(Some(selectable[selected].clone()));
            }
            _ => {}
        }
    }
}

pub fn select_agent_line<R: BufRead, W: Write>(
    input: &mut R,
    out: &mut W,
    choices: &[AgentChoice],
    default_agent: Option<&str>,
) -> io::Result<Option<AgentChoice>> {
    let selectable = selectable_agents(choices);
    if selectable.is_empty() {
        render_no_runnable_agents(out)?;
        return Ok(None);
    }

    render_line_home(out, choices, default_agent)?;
    render_agent_table(out, choices, default_agent)?;

    loop {
        write!(out, "{CYAN}Select agent{RESET} ")?;
        if let Some(default) = default_agent {
            write!(out, "{DIM}[{default}]{RESET} ")?;
        }
        write!(out, "> ")?;
        out.flush()?;

        let Some(line) = read_line(input)? else {
            return Ok(None);
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some(default) = default_agent {
                if let Some(choice) = selectable.iter().find(|choice| choice.name == default) {
                    writeln!(out)?;
                    return Ok(Some((*choice).clone()));
                }
            }
            writeln!(out)?;
            return Ok(Some(selectable[0].clone()));
        }
        if is_quit(trimmed) {
            writeln!(out)?;
            return Ok(None);
        }

        if let Ok(n) = trimmed.parse::<usize>() {
            if (1..=selectable.len()).contains(&n) {
                writeln!(out)?;
                return Ok(Some(selectable[n - 1].clone()));
            }
            writeln!(
                out,
                "{YELLOW}Choose a number from 1 to {}.{RESET}",
                selectable.len()
            )?;
            continue;
        }

        if let Some(choice) = selectable
            .iter()
            .find(|choice| choice.name.eq_ignore_ascii_case(trimmed))
        {
            writeln!(out)?;
            return Ok(Some((*choice).clone()));
        }

        writeln!(
            out,
            "{YELLOW}Unknown or unavailable agent: {trimmed}. Type a listed name, number, or /quit.{RESET}"
        )?;
    }
}

pub fn read_prompt_action<R: BufRead, W: Write>(
    input: &mut R,
    out: &mut W,
    agent: &AgentChoice,
    resume: Option<&str>,
    choices: &[AgentChoice],
) -> io::Result<PromptAction> {
    write!(out, "{CYAN}{}>{RESET} ", agent.name)?;
    if resume.is_some() {
        write!(out, "{DIM}resume{RESET} ")?;
    }
    out.flush()?;

    let Some(line) = read_line(input)? else {
        return Ok(PromptAction::Quit);
    };
    Ok(parse_prompt_action(line.trim(), choices))
}

pub fn parse_prompt_action(input: &str, choices: &[AgentChoice]) -> PromptAction {
    if input.is_empty() {
        return PromptAction::Empty;
    }
    if input == "?" {
        return PromptAction::Help;
    }
    if !input.starts_with('/') {
        return PromptAction::Run(input.to_string());
    }

    match command_action(input, matching_commands(input, choices).first()) {
        PromptAction::Empty => PromptAction::Help,
        action => action,
    }
}

pub fn parse_home_action(input: &str, choices: &[AgentChoice]) -> HomeAction {
    parse_home_action_selected(input, choices, 0)
}

fn parse_home_action_selected(input: &str, choices: &[AgentChoice], selected: usize) -> HomeAction {
    let input = input.trim();
    if input.is_empty() {
        return HomeAction::Empty;
    }
    if !input.starts_with('/') {
        return HomeAction::Submit(input.to_owned());
    }
    if matches!(input, "/delegate" | "/agents") {
        return HomeAction::PickAgent;
    }

    let matches = matching_commands(input, choices);
    let selected_item = matches.get(selected);
    let mut action = command_action(input, selected_item);
    if matches!(action, PromptAction::Empty) && selected_item.is_some() {
        action = command_action("/", selected_item);
    }

    match action {
        PromptAction::PickAgent => HomeAction::PickAgent,
        PromptAction::ListAgents => HomeAction::PickAgent,
        PromptAction::LaunchAgent(name) => HomeAction::LaunchAgent(name),
        PromptAction::Help => HomeAction::Help,
        PromptAction::Quit => HomeAction::Quit,
        PromptAction::Empty => HomeAction::Help,
        PromptAction::Run(prompt) => HomeAction::Submit(prompt),
    }
}

pub fn render_agent_table<W: Write>(
    out: &mut W,
    choices: &[AgentChoice],
    default_agent: Option<&str>,
) -> io::Result<()> {
    let selectable = selectable_agents(choices);

    writeln!(out, "{BOLD}Available agents{RESET}")?;
    for (i, choice) in selectable.iter().enumerate() {
        let default = if Some(choice.name.as_str()) == default_agent {
            " *"
        } else {
            ""
        };
        writeln!(
            out,
            "  {CYAN}{:>2}{RESET}. {BOLD}{:<10}{RESET} {:<8} {:<18} {}{}",
            i + 1,
            choice.name,
            status_label(choice.status),
            choice.kinds,
            launch_label(choice),
            default
        )?;
        writeln!(out, "      {DIM}{}{RESET}", choice.detail)?;
    }

    let unavailable = choices
        .iter()
        .filter(|choice| !choice.is_available())
        .collect::<Vec<_>>();
    if !unavailable.is_empty() {
        writeln!(out)?;
        writeln!(out, "{BOLD}Not available on this PATH{RESET}")?;
        for choice in unavailable {
            writeln!(
                out,
                "  {DIM}{:<10} {:<18} {}{RESET}",
                choice.name, choice.kinds, choice.detail
            )?;
        }
    }
    writeln!(out)?;
    writeln!(
        out,
        "{DIM}Commands: /agent switch, /list agents, /help help, /quit exit.{RESET}"
    )
}

pub fn render_help<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{BOLD}Interactive commands{RESET}")?;
    writeln!(out, "  /agent   choose another installed agent")?;
    writeln!(out, "  /list    show detected agent status")?;
    writeln!(out, "  /help    show this help")?;
    writeln!(out, "  /quit    exit Orchester")?;
    writeln!(out, "  /codex   launch Codex CLI when installed")?;
    writeln!(
        out,
        "  text     send a task through Orchester's headless adapter"
    )?;
    writeln!(out)
}

pub fn render_run_header<W: Write>(
    out: &mut W,
    agent: &AgentChoice,
    resume: Option<&str>,
) -> io::Result<()> {
    writeln!(out)?;
    match resume {
        Some(session) => writeln!(
            out,
            "{DIM}Running {} with resumed session {}...{RESET}",
            agent.name, session
        ),
        None => writeln!(out, "{DIM}Running {}...{RESET}", agent.name),
    }
}

pub fn render_run_footer<W: Write>(
    out: &mut W,
    outcome: orchester_protokoll::Outcome,
    input_tokens: u64,
    output_tokens: u64,
) -> io::Result<()> {
    writeln!(
        out,
        "{DIM}-> {:?} | tokens in {} / out {}{RESET}",
        outcome, input_tokens, output_tokens
    )?;
    writeln!(out)
}

#[cfg(test)]
fn render_home<W: Write>(
    out: &mut W,
    choices: &[AgentChoice],
    selected: usize,
    command: &str,
    command_selected: usize,
    message: &str,
) -> io::Result<()> {
    render_home_frame(
        out,
        PickerView {
            width: 100,
            height: usize::MAX,
            choices,
            selected,
            command,
            command_selected,
            message,
        },
    )
}

fn present_home<W: Write>(
    presenter: &mut FramePresenter,
    out: &mut W,
    view: PickerView<'_>,
) -> io::Result<()> {
    let mut frame = Vec::new();
    render_home_frame(&mut frame, view)?;
    presenter.present(out, &frame)
}

fn render_home_frame<W: Write>(out: &mut W, view: PickerView<'_>) -> io::Result<()> {
    let PickerView {
        width,
        height,
        choices,
        selected,
        command,
        command_selected,
        message,
    } = view;
    if height == 0 {
        return Ok(());
    }
    let selectable = selectable_agents(choices);
    let selected = selected.min(selectable.len().saturating_sub(1));
    let selected_agent = selectable.get(selected);
    let width = width.clamp(2, 132);

    if command.starts_with('/') {
        return render_picker_command_frame(out, width, height, command, choices, command_selected);
    }

    let full_header = width >= 30 && height >= PICKER_PANEL_ROWS + 4;
    let header_rows = if full_header {
        render_delegate_panel(out, width, selected_agent)?;
        writeln!(out)?;
        writeln!(out, "{BOLD}Choose agent{RESET}")?;
        PICKER_PANEL_ROWS + 2
    } else {
        let rows = if height >= 4 {
            2
        } else if height >= 2 {
            1
        } else {
            0
        };
        render_compact_picker_header(out, width, rows, selected_agent)?;
        rows
    };

    let footer_rows = usize::from(height > header_rows.saturating_add(1));
    let list_rows = height
        .saturating_sub(header_rows)
        .saturating_sub(footer_rows);
    let start = selection_window_start(selected, selectable.len(), list_rows);
    for (index, choice) in selectable.iter().enumerate().skip(start).take(list_rows) {
        render_picker_agent_row(out, width, index == selected, choice)?;
    }
    if footer_rows > 0 {
        render_picker_footer(out, width, selected_agent, choices, message)?;
    }
    Ok(())
}

fn render_picker_command_frame<W: Write>(
    out: &mut W,
    width: usize,
    height: usize,
    command: &str,
    choices: &[AgentChoice],
    command_selected: usize,
) -> io::Result<()> {
    let header_rows = usize::from(height >= 5);
    let status_rows = usize::from(height >= 3);
    if header_rows > 0 {
        writeln!(out, "{BOLD}Agent commands{RESET}")?;
    }
    let prompt = truncate(&sanitize_terminal_text(&format!("> {command}")), width);
    writeln!(out, "{BOLD}{prompt}{RESET}")?;
    let palette_rows = height
        .saturating_sub(header_rows)
        .saturating_sub(1)
        .saturating_sub(status_rows)
        .min(PALETTE_ROWS);
    render_command_palette(out, command, choices, command_selected, palette_rows, width)?;
    if status_rows > 0 {
        write!(
            out,
            "{DIM}{}{RESET}",
            truncate("Up/Down selects; Enter opens; Esc returns", width)
        )?;
    }
    Ok(())
}

fn render_compact_picker_header<W: Write>(
    out: &mut W,
    width: usize,
    rows: usize,
    selected_agent: Option<&AgentChoice>,
) -> io::Result<()> {
    if rows > 0 {
        writeln!(out, "{BOLD}{}{RESET}", truncate("Choose agent", width))?;
    }
    if rows > 1 {
        let selected = selected_agent
            .map(|choice| format!("Selected: {} ({})", choice.name, launch_label(choice)))
            .unwrap_or_else(|| "Selected: none".to_string());
        writeln!(
            out,
            "{DIM}{}{RESET}",
            truncate(&sanitize_terminal_text(&selected), width)
        )?;
    }
    Ok(())
}

fn render_picker_agent_row<W: Write>(
    out: &mut W,
    width: usize,
    selected: bool,
    choice: &AgentChoice,
) -> io::Result<()> {
    let marker = if selected { ">" } else { " " };
    let color = if selected { CYAN } else { "" };
    let reset = if selected { RESET } else { "" };
    let name = truncate(&sanitize_terminal_text(&choice.name), 12);
    let name_pad = " ".repeat(12usize.saturating_sub(display_width(&name)));
    let kinds = sanitize_terminal_text(&choice.kinds);
    let line = format!(
        "{marker} {name}{name_pad} {:<7} {kinds} {}",
        plain_status(choice.status),
        launch_label(choice)
    );
    writeln!(out, "{color}{}{reset}", truncate(&line, width))
}

fn render_picker_footer<W: Write>(
    out: &mut W,
    width: usize,
    selected_agent: Option<&AgentChoice>,
    choices: &[AgentChoice],
    message: &str,
) -> io::Result<()> {
    let (style, footer) = if message.is_empty() {
        let detail = selected_agent
            .map(|choice| sanitize_terminal_text(&choice.detail))
            .unwrap_or_else(|| "No runnable agent".to_string());
        let unavailable = choices
            .iter()
            .filter(|choice| !choice.is_available())
            .count();
        (
            DIM,
            format!(
                "{detail} | {unavailable} unavailable | Enter launches; / commands; Esc returns"
            ),
        )
    } else {
        (YELLOW, sanitize_terminal_text(message))
    };
    write!(out, "{style}{}{RESET}", truncate(&footer, width))
}

#[cfg(test)]
fn render_chat_home<W: Write>(
    out: &mut W,
    width: usize,
    input: &str,
    choices: &[AgentChoice],
    command_selected: usize,
    show_help: bool,
) -> io::Result<()> {
    render_chat_home_in_viewport(
        out,
        width,
        usize::MAX,
        input,
        choices,
        command_selected,
        show_help,
    )
}

#[cfg(test)]
fn render_chat_home_in_viewport<W: Write>(
    out: &mut W,
    width: usize,
    height: usize,
    input: &str,
    choices: &[AgentChoice],
    command_selected: usize,
    show_help: bool,
) -> io::Result<()> {
    render_chat_home_frame(
        out,
        width,
        height,
        input,
        choices,
        command_selected,
        show_help,
    )
}

#[allow(clippy::too_many_arguments)]
fn present_chat_home_in_viewport<W: Write>(
    presenter: &mut FramePresenter,
    out: &mut W,
    width: usize,
    height: usize,
    input: &str,
    choices: &[AgentChoice],
    command_selected: usize,
    show_help: bool,
) -> io::Result<()> {
    let mut frame = Vec::new();
    render_chat_home_frame(
        &mut frame,
        width,
        height,
        input,
        choices,
        command_selected,
        show_help,
    )?;
    presenter.present(out, &frame)
}

fn render_chat_home_frame<W: Write>(
    out: &mut W,
    width: usize,
    height: usize,
    input: &str,
    choices: &[AgentChoice],
    command_selected: usize,
    show_help: bool,
) -> io::Result<()> {
    if height == 0 {
        return Ok(());
    }

    let desired_content_rows = desired_home_content_rows(width, input, choices, show_help);
    let status_rows = usize::from(height >= 2);
    let prompt_rows = 1;
    let full_panel_rows = chat_panel_line_count(width);
    let full_panel_total = full_panel_rows
        .saturating_add(1)
        .saturating_add(prompt_rows)
        .saturating_add(desired_content_rows)
        .saturating_add(status_rows);

    let (header_rows, separator_rows, full_panel) = if width >= 50 && full_panel_total <= height {
        (full_panel_rows, 1, true)
    } else {
        let minimum_body = prompt_rows
            .saturating_add(status_rows)
            .saturating_add(usize::from(desired_content_rows > 0));
        match height.saturating_sub(minimum_body) {
            remaining if remaining >= 3 => (2, 1, false),
            remaining if remaining >= 1 => (1, 0, false),
            _ => (0, 0, false),
        }
    };

    if full_panel {
        render_chat_panel(out, width)?;
    } else {
        render_compact_home_header(out, width, header_rows)?;
    }
    if separator_rows > 0 {
        writeln!(out)?;
    }

    let (prompt, prompt_style) = if input.is_empty() {
        (prompt_suggestion(), DIM)
    } else {
        (input, "")
    };
    let prompt = truncate(&sanitize_terminal_text(prompt), width.saturating_sub(2));
    let prompt_pad = " ".repeat(width.saturating_sub(2 + display_width(&prompt)));
    writeln!(
        out,
        "\x1b[48;5;236m{CYAN}> {RESET}\x1b[48;5;236m{prompt_style}{prompt}{prompt_pad}{RESET}"
    )?;

    let content_rows = height
        .saturating_sub(header_rows)
        .saturating_sub(separator_rows)
        .saturating_sub(prompt_rows)
        .saturating_sub(status_rows);
    if show_help {
        render_home_help(out, width, content_rows)?;
    } else if input.starts_with('/') {
        if width < 50 {
            render_compact_command_palette(
                out,
                input,
                choices,
                command_selected,
                width,
                content_rows.min(COMPACT_PALETTE_ROWS),
            )?;
        } else {
            render_command_palette(
                out,
                input,
                choices,
                command_selected,
                content_rows.min(PALETTE_ROWS),
                width,
            )?;
        }
    } else if content_rows > 0 {
        let hint = truncate(
            "Type a task or / for commands. Enter submits; Esc exits.",
            width,
        );
        writeln!(out, "{DIM}{hint}{RESET}")?;
    }
    if status_rows > 0 {
        render_status_line(out, width)?;
    }
    Ok(())
}

fn desired_home_content_rows(
    width: usize,
    input: &str,
    choices: &[AgentChoice],
    show_help: bool,
) -> usize {
    if show_help {
        return 6;
    }
    if input.starts_with('/') {
        let matches = matching_commands(input, choices).len();
        let limit = if width < 50 {
            COMPACT_PALETTE_ROWS
        } else {
            PALETTE_ROWS
        };
        return matches.max(1).min(limit);
    }
    1
}

fn render_compact_home_header<W: Write>(out: &mut W, width: usize, rows: usize) -> io::Result<()> {
    if rows > 0 {
        writeln!(
            out,
            "{ORANGE}{BOLD}Orchester{RESET} {DIM}v{}{RESET}",
            env!("CARGO_PKG_VERSION")
        )?;
    }
    if rows > 1 {
        writeln!(
            out,
            "{DIM}{} {RESET}",
            truncate("coding agent workspace", width)
        )?;
    }
    Ok(())
}

fn render_home_help<W: Write>(out: &mut W, width: usize, max_rows: usize) -> io::Result<()> {
    for line in [
        "/agent      choose a delegate",
        "/codex      launch Codex",
        "/claude     launch Claude",
        "/opencode   launch OpenCode",
        "/quit       exit Orchester",
        "Esc         close help",
    ]
    .into_iter()
    .take(max_rows)
    {
        writeln!(out, "{}", truncate(line, width))?;
    }
    Ok(())
}

fn render_compact_command_palette<W: Write>(
    out: &mut W,
    command: &str,
    choices: &[AgentChoice],
    selected: usize,
    width: usize,
    max_rows: usize,
) -> io::Result<()> {
    let matches = matching_commands(command, choices);
    let start = selection_window_start(selected, matches.len(), max_rows);
    for (index, item) in matches.iter().enumerate().skip(start).take(max_rows) {
        let marker = if index == selected { ">" } else { " " };
        let line = sanitize_terminal_text(&format!("{marker} {} {}", item.name, item.description));
        writeln!(out, "{}", truncate(&line, width))?;
    }
    Ok(())
}

fn render_line_home<W: Write>(
    out: &mut W,
    choices: &[AgentChoice],
    default_agent: Option<&str>,
) -> io::Result<()> {
    let selectable = selectable_agents(choices);
    let selected = default_index(&selectable, default_agent);
    let selected_agent = selectable.get(selected);
    let (cols, _) = terminal::size().unwrap_or((100, 30));
    let width = (cols as usize).clamp(50, 132);

    writeln!(out)?;
    render_delegate_panel(out, width, selected_agent)?;
    writeln!(out)
}

pub fn render_line_startup_home<W: Write>(out: &mut W) -> io::Result<()> {
    let (cols, _) = terminal::size().unwrap_or((100, 30));
    let width = (cols as usize).clamp(50, 132);

    render_chat_panel(out, width)?;
    writeln!(out)?;
    writeln!(
        out,
        "{DIM}Type a task for Orchester, or /agent, /codex, /claude, /opencode.{RESET}"
    )?;
    writeln!(
        out,
        "{DIM}Use `orchester run --agent <name> <prompt>` for scripts.{RESET}"
    )?;
    write!(out, "{CYAN}orchester>{RESET} ")?;
    out.flush()
}

fn render_chat_panel<W: Write>(out: &mut W, width: usize) -> io::Result<()> {
    let rows = startup_panel_rows();
    if width >= 60 {
        render_portrait_info_box(out, width, &rows)
    } else {
        render_info_box(out, width, &rows)
    }
}

fn startup_panel_rows() -> Vec<String> {
    let cwd = current_directory_text();
    vec![
        format!(">_ Orchester (v{})", env!("CARGO_PKG_VERSION")),
        "Self-owned coding agent workspace".to_string(),
        String::new(),
        "Getting started".to_string(),
        prompt_suggestion().to_string(),
        String::new(),
        "Workspace".to_string(),
        format!("directory: {cwd}"),
        "model: not configured".to_string(),
        "safety: governed".to_string(),
        String::new(),
        "Delegate agents".to_string(),
        "/agent choose or switch agent".to_string(),
        "/codex launch native Codex".to_string(),
        "/claude launch native Claude".to_string(),
        "/opencode launch native OpenCode".to_string(),
        String::new(),
        "Recent activity".to_string(),
        "No recent activity".to_string(),
    ]
}

fn chat_panel_line_count(width: usize) -> usize {
    let info_rows = startup_panel_rows().len();
    if width >= 60 {
        avatar::HEIGHT.max(info_rows).saturating_add(2)
    } else {
        info_rows.saturating_add(2)
    }
}

fn render_delegate_panel<W: Write>(
    out: &mut W,
    width: usize,
    selected_agent: Option<&AgentChoice>,
) -> io::Result<()> {
    let selected = selected_agent
        .map(|agent| format!("{} ({})", agent.name, launch_label(agent)))
        .unwrap_or_else(|| "none".to_string());
    let rows = vec![
        "Delegated agent".to_string(),
        String::new(),
        format!("Selected: {selected}"),
        format!("directory: {}", current_directory_text()),
        "Enter launches; Esc returns to Orchester".to_string(),
    ];
    render_info_box(out, width, &rows)
}

fn render_info_box<W: Write>(out: &mut W, width: usize, rows: &[String]) -> io::Result<()> {
    let panel_width = width.clamp(20, 84);
    let content_width = panel_width.saturating_sub(4);
    writeln!(out, "{DIM}+{}+{RESET}", "-".repeat(panel_width - 2))?;
    for row in rows {
        let row = truncate(&sanitize_terminal_text(row), content_width);
        let pad = " ".repeat(content_width.saturating_sub(display_width(&row)));
        writeln!(out, "{DIM}|{RESET} {row}{pad} {DIM}|{RESET}")?;
    }
    writeln!(out, "{DIM}+{}+{RESET}", "-".repeat(panel_width - 2))
}

fn render_portrait_info_box<W: Write>(
    out: &mut W,
    width: usize,
    rows: &[String],
) -> io::Result<()> {
    let panel_width = width.clamp(60, 120);
    let portrait_width = if panel_width >= 96 { avatar::WIDTH } else { 24 };
    let right_width = panel_width.saturating_sub(portrait_width + 7);
    let height = avatar::HEIGHT.max(rows.len());

    writeln!(out, "{DIM}+{}+{RESET}", "-".repeat(panel_width - 2))?;
    for row in 0..height {
        write!(out, "{DIM}|{RESET} ")?;
        if portrait_width == avatar::WIDTH {
            avatar::render_row(out, row)?;
        } else {
            avatar::render_row_width(out, row, portrait_width)?;
        }
        write!(out, " {DIM}|{RESET} ")?;

        let text = rows.get(row).map(String::as_str).unwrap_or("");
        let text = truncate(&sanitize_terminal_text(text), right_width);
        let pad = " ".repeat(right_width.saturating_sub(display_width(&text)));
        write!(out, "{text}{pad} ")?;
        writeln!(out, "{DIM}|{RESET}")?;
    }
    writeln!(out, "{DIM}+{}+{RESET}", "-".repeat(panel_width - 2))
}

fn render_status_line<W: Write>(out: &mut W, width: usize) -> io::Result<()> {
    let status = format!(
        "{}  |  model not configured  |  governed workspace",
        current_directory_text()
    );
    write!(out, "{DIM}{}{RESET}", truncate(&status, width))
}

fn viewport_content_width(cols: u16) -> usize {
    usize::from(cols).saturating_sub(1).clamp(2, 132)
}

fn current_directory_text() -> String {
    std::env::current_dir()
        .map(|cwd| sanitize_terminal_text(&cwd.display().to_string()))
        .unwrap_or_else(|_| ".".into())
}

fn prompt_suggestion() -> &'static str {
    static SUGGESTION: OnceLock<&'static str> = OnceLock::new();

    SUGGESTION.get_or_init(|| {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let entropy = timestamp ^ u128::from(std::process::id());
        let index = (entropy % PROMPT_SUGGESTIONS.len() as u128) as usize;
        PROMPT_SUGGESTIONS[index]
    })
}

fn render_command_palette<W: Write>(
    out: &mut W,
    command: &str,
    choices: &[AgentChoice],
    selected: usize,
    max_rows: usize,
    width: usize,
) -> io::Result<()> {
    if max_rows == 0 {
        return Ok(());
    }
    let matches = matching_commands(command, choices);
    if matches.is_empty() {
        writeln!(out, "  {DIM}No matching commands{RESET}")?;
        return Ok(());
    }
    let start = selection_window_start(selected, matches.len(), max_rows);
    for (i, item) in matches.iter().enumerate().skip(start).take(max_rows) {
        let marker = if i == selected { ">" } else { " " };
        let color = if i == selected { ORANGE } else { "" };
        let reset = if i == selected { RESET } else { "" };
        let name = truncate(&sanitize_terminal_text(&item.name), 16);
        let name_pad = " ".repeat(16usize.saturating_sub(display_width(&name)));
        let description = sanitize_terminal_text(&item.description);
        let line = truncate(&format!("{marker} {name}{name_pad} {description}"), width);
        writeln!(out, "{color}{line}{reset}")?;
    }
    Ok(())
}

fn render_no_runnable_agents<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(out, "{RED}No runnable agents were found.{RESET}")
}

fn read_line<R: BufRead>(input: &mut R) -> io::Result<Option<String>> {
    let mut line = String::new();
    let read = input.read_line(&mut line)?;
    if read == 0 {
        Ok(None)
    } else {
        Ok(Some(line))
    }
}

pub fn read_startup_line<R: BufRead>(input: &mut R) -> io::Result<Option<String>> {
    read_line(input)
}

fn selectable_agents(choices: &[AgentChoice]) -> Vec<AgentChoice> {
    choices
        .iter()
        .filter(|choice| choice.is_available())
        .cloned()
        .collect()
}

fn default_index(choices: &[AgentChoice], default_agent: Option<&str>) -> usize {
    default_agent
        .and_then(|default| choices.iter().position(|choice| choice.name == default))
        .unwrap_or(0)
}

fn matching_commands(query: &str, choices: &[AgentChoice]) -> Vec<CommandItem> {
    let normalized = query
        .split_whitespace()
        .next()
        .unwrap_or(query)
        .trim_start_matches('/')
        .to_ascii_lowercase();
    command_items(choices)
        .into_iter()
        .filter(|item| {
            let name = item.name.trim_start_matches('/').to_ascii_lowercase();
            normalized.is_empty()
                || name.starts_with(&normalized)
                || item.description.to_ascii_lowercase().contains(&normalized)
        })
        .collect()
}

fn command_action(input: &str, selected: Option<&CommandItem>) -> PromptAction {
    let token = input
        .split_whitespace()
        .next()
        .unwrap_or(input)
        .trim()
        .to_ascii_lowercase();
    match token.as_str() {
        "/a" | "/agent" => return PromptAction::PickAgent,
        "/l" | "/list" | "/agents" | "/doctor" => return PromptAction::ListAgents,
        "/h" | "/help" => return PromptAction::Help,
        "/q" | "/quit" | "/exit" => return PromptAction::Quit,
        _ => {}
    }
    let item = if token == "/" || token.is_empty() {
        selected
    } else {
        selected.filter(|candidate| candidate.name.eq_ignore_ascii_case(&token))
    };

    let Some(item) = item else {
        return PromptAction::Empty;
    };
    match item.action {
        CommandAction::PickAgent => PromptAction::PickAgent,
        CommandAction::ListAgents => PromptAction::ListAgents,
        CommandAction::Help => PromptAction::Help,
        CommandAction::Quit => PromptAction::Quit,
        CommandAction::LaunchAgent => item
            .agent
            .clone()
            .map(PromptAction::LaunchAgent)
            .unwrap_or(PromptAction::Empty),
    }
}

fn command_items(choices: &[AgentChoice]) -> Vec<CommandItem> {
    let mut items = vec![
        CommandItem {
            name: "/agent".into(),
            description: "choose or switch agent".into(),
            action: CommandAction::PickAgent,
            agent: None,
        },
        CommandItem {
            name: "/list".into(),
            description: "show detected agent status".into(),
            action: CommandAction::ListAgents,
            agent: None,
        },
        CommandItem {
            name: "/doctor".into(),
            description: "refresh local availability checks".into(),
            action: CommandAction::ListAgents,
            agent: None,
        },
        CommandItem {
            name: "/help".into(),
            description: "show interactive commands".into(),
            action: CommandAction::Help,
            agent: None,
        },
        CommandItem {
            name: "/quit".into(),
            description: "exit Orchester".into(),
            action: CommandAction::Quit,
            agent: None,
        },
    ];
    for choice in choices.iter().filter(|choice| choice.is_available()) {
        items.push(CommandItem {
            name: format!("/{}", choice.name),
            description: match &choice.native_command {
                Some(command) => format!("launch native {command}"),
                None => "use built-in Orchester adapter".into(),
            },
            action: CommandAction::LaunchAgent,
            agent: Some(choice.name.clone()),
        });
    }
    items
}

fn render_kinds(cap: &Capability) -> String {
    if cap.kinds.is_empty() {
        return "-".into();
    }
    cap.kinds
        .iter()
        .map(kind_word)
        .collect::<Vec<_>>()
        .join(",")
}

fn kind_word(kind: &TaskKind) -> String {
    match kind {
        TaskKind::Code => "code".into(),
        TaskKind::Review => "review".into(),
        TaskKind::Chat => "chat".into(),
        TaskKind::Browser => "browser".into(),
        TaskKind::Custom(s) => s.clone(),
    }
}

fn status_label(status: AvailabilityStatus) -> String {
    match status {
        AvailabilityStatus::Available => format!("{GREEN}ready{RESET}"),
        AvailabilityStatus::Unknown => format!("{YELLOW}unknown{RESET}"),
        AvailabilityStatus::Missing => format!("{RED}missing{RESET}"),
    }
}

fn plain_status(status: AvailabilityStatus) -> &'static str {
    match status {
        AvailabilityStatus::Available => "ready",
        AvailabilityStatus::Unknown => "unknown",
        AvailabilityStatus::Missing => "missing",
    }
}

fn launch_label(choice: &AgentChoice) -> &'static str {
    if choice.native_command.is_some() {
        "native"
    } else if choice.supports_resume {
        "adapter resume"
    } else {
        "adapter"
    }
}

fn status_rank(status: AvailabilityStatus) -> u8 {
    match status {
        AvailabilityStatus::Available => 0,
        AvailabilityStatus::Unknown => 1,
        AvailabilityStatus::Missing => 2,
    }
}

fn native_rank(choice: &AgentChoice) -> u8 {
    if choice.native_command.is_some() {
        0
    } else {
        1
    }
}

fn agent_rank(name: &str) -> u8 {
    match name {
        "codex" => 0,
        "claude" => 1,
        "opencode" => 2,
        "mock" => 200,
        _ => 100,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if display_width(s) <= max {
        return s.to_string();
    }

    let suffix = if max >= 3 { "..." } else { "" };
    let budget = max.saturating_sub(display_width(suffix));
    let mut width = 0;
    let mut out = String::new();
    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > budget {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out.push_str(suffix);
    out
}

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn sanitize_terminal_text(s: &str) -> String {
    s.chars()
        .flat_map(|ch| {
            if ch.is_control() {
                ch.escape_default().collect::<Vec<_>>()
            } else {
                vec![ch]
            }
        })
        .collect()
}

fn is_quit(input: &str) -> bool {
    matches!(input, "/quit" | "/exit" | "/q" | "quit" | "exit" | "q")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn select_agent_accepts_name() {
        let choices = vec![
            choice("codex", AvailabilityStatus::Missing, Some("codex")),
            choice("mock", AvailabilityStatus::Available, None),
        ];
        let mut input = Cursor::new(b"mock\n");
        let mut out = Vec::new();

        let selected = select_agent_line(&mut input, &mut out, &choices, None)
            .unwrap()
            .unwrap();

        assert_eq!(selected.name, "mock");
    }

    #[test]
    fn select_agent_skips_missing_numbering() {
        let choices = vec![
            choice("codex", AvailabilityStatus::Missing, Some("codex")),
            choice("mock", AvailabilityStatus::Available, None),
        ];
        let mut input = Cursor::new(b"1\n");
        let mut out = Vec::new();

        let selected = select_agent_line(&mut input, &mut out, &choices, None)
            .unwrap()
            .unwrap();

        assert_eq!(selected.name, "mock");
    }

    #[test]
    fn prompt_action_parses_agent_switch_with_argument() {
        let choices = vec![choice("mock", AvailabilityStatus::Available, None)];

        assert_eq!(
            parse_prompt_action("/agent switch", &choices),
            PromptAction::PickAgent
        );
    }

    #[test]
    fn prompt_action_parses_command_aliases() {
        let choices = vec![choice("mock", AvailabilityStatus::Available, None)];

        assert_eq!(parse_prompt_action("/q", &choices), PromptAction::Quit);
        assert_eq!(parse_prompt_action("/exit", &choices), PromptAction::Quit);
        assert_eq!(
            parse_prompt_action("/agents", &choices),
            PromptAction::ListAgents
        );
        assert_eq!(
            parse_prompt_action("/doctor", &choices),
            PromptAction::ListAgents
        );
        assert_eq!(parse_prompt_action("?", &choices), PromptAction::Help);
    }

    #[test]
    fn prompt_action_parses_dynamic_agent_command() {
        let choices = vec![choice(
            "codex",
            AvailabilityStatus::Available,
            Some("codex"),
        )];

        assert_eq!(
            parse_prompt_action("/codex", &choices),
            PromptAction::LaunchAgent("codex".into())
        );
    }

    #[test]
    fn prompt_action_runs_plain_text() {
        let choices = vec![choice("mock", AvailabilityStatus::Available, None)];
        let agent = choices[0].clone();
        let mut input = Cursor::new(b"write tests\n");
        let mut out = Vec::new();

        let action =
            read_prompt_action(&mut input, &mut out, &agent, Some("sid"), &choices).unwrap();

        assert_eq!(action, PromptAction::Run("write tests".into()));
    }

    #[test]
    fn delegate_home_renders_explicit_selection_without_brand_art() {
        let choices = vec![choice(
            "codex",
            AvailabilityStatus::Available,
            Some("codex"),
        )];
        let mut out = Vec::new();

        render_home(&mut out, &choices, 0, "", 0, "").unwrap();

        let rendered = String::from_utf8_lossy(&out);
        let plain = strip_ansi(&rendered);
        assert!(
            plain.contains("Delegated agent"),
            "home output:\n{rendered}"
        );
        assert!(
            plain.contains("Selected: codex"),
            "home output:\n{rendered}"
        );
        assert!(!plain.contains("/#\\"), "home output:\n{rendered}");
    }

    #[test]
    fn delegate_picker_respects_the_viewport_and_keeps_selection_visible() {
        let choices = (0..20)
            .map(|index| {
                choice(
                    &format!("worker{index}"),
                    AvailabilityStatus::Available,
                    Some("worker"),
                )
            })
            .collect::<Vec<_>>();
        let mut out = Vec::new();

        render_home_frame(
            &mut out,
            PickerView {
                width: 40,
                height: 8,
                choices: &choices,
                selected: 19,
                command: "",
                command_selected: 0,
                message: "",
            },
        )
        .unwrap();

        let plain = strip_ansi(&String::from_utf8(out).unwrap());
        assert!(
            plain.lines().count() <= 8,
            "picker exceeded its row budget:\n{plain}"
        );
        assert!(
            plain.lines().all(|line| display_width(line) <= 40),
            "picker exceeded its column budget:\n{plain}"
        );
        assert!(
            plain.lines().any(|line| line.contains("> worker19")),
            "selected agent is not visible:\n{plain}"
        );
    }

    #[test]
    fn delegate_picker_command_palette_stays_bounded_and_visible() {
        let choices = (0..20)
            .map(|index| {
                choice(
                    &format!("worker{index}"),
                    AvailabilityStatus::Available,
                    Some("worker"),
                )
            })
            .collect::<Vec<_>>();
        let selected = matching_commands("/", &choices).len() - 1;
        let mut out = Vec::new();

        render_home_frame(
            &mut out,
            PickerView {
                width: 40,
                height: 8,
                choices: &choices,
                selected: 19,
                command: "/",
                command_selected: selected,
                message: "",
            },
        )
        .unwrap();

        let plain = strip_ansi(&String::from_utf8(out).unwrap());
        assert!(
            plain.lines().count() <= 8,
            "picker palette exceeded its row budget:\n{plain}"
        );
        assert!(
            plain.lines().all(|line| display_width(line) <= 40),
            "picker palette exceeded its column budget:\n{plain}"
        );
        assert!(
            plain.lines().any(|line| line.contains("> /worker19")),
            "selected command is not visible:\n{plain}"
        );
    }

    #[test]
    fn leaving_delegate_commands_replaces_the_palette_without_global_clear() {
        let choices = (0..8)
            .map(|index| {
                choice(
                    &format!("worker{index}"),
                    AvailabilityStatus::Available,
                    Some("worker"),
                )
            })
            .collect::<Vec<_>>();
        let selected_command = matching_commands("/", &choices).len() - 1;
        let mut presenter = FramePresenter::default();
        let mut out = Vec::new();
        present_home(
            &mut presenter,
            &mut out,
            PickerView {
                width: 40,
                height: 8,
                choices: &choices,
                selected: 7,
                command: "/",
                command_selected: selected_command,
                message: "",
            },
        )
        .unwrap();
        out.clear();

        present_home(
            &mut presenter,
            &mut out,
            PickerView {
                width: 40,
                height: 8,
                choices: &choices,
                selected: 7,
                command: "",
                command_selected: 0,
                message: "",
            },
        )
        .unwrap();

        let update = String::from_utf8(out).unwrap();
        assert!(!update.contains("/worker7"));
        assert!(!update.contains("\x1b[J"));
        assert!(!update.contains("\x1b[2J"));
        assert!(update.contains("worker7"));
    }

    #[test]
    fn startup_home_is_distinct_from_the_delegate_picker() {
        let mut out = Vec::new();

        render_chat_home(&mut out, 100, "", &[], 0, false).unwrap();

        let rendered = String::from_utf8_lossy(&out);
        let plain = strip_ansi(&rendered);
        assert!(
            plain.contains(">_ Orchester"),
            "startup output:\n{rendered}"
        );
        assert!(plain.contains("model:"), "startup output:\n{rendered}");
        assert!(plain.contains("directory:"), "startup output:\n{rendered}");
        assert!(
            plain.contains("Type a task or / for commands"),
            "startup output:\n{rendered}"
        );
        assert!(
            !plain.contains("Selected: codex") && !plain.contains("Choose agent"),
            "startup must not look like a Codex session or delegate picker:\n{rendered}"
        );
        assert!(
            rendered.contains("\x1b[38;2;"),
            "wide startup should render the true-colour logo portrait:\n{rendered}"
        );
        assert!(
            plain
                .chars()
                .filter(|ch| *ch == '\u{2580}')
                .count()
                > 40,
            "startup portrait should be recognisable as dense ANSI art:\n{rendered}"
        );
        assert!(
            !plain.contains("\u{923b}\u{20ac}"),
            "startup portrait must not expose the source file's mojibake:\n{rendered}"
        );
    }

    #[test]
    fn startup_home_offers_a_prompt_and_workspace_context() {
        let mut out = Vec::new();

        render_chat_home(&mut out, 100, "", &[], 0, false).unwrap();

        let rendered = String::from_utf8(out).unwrap();
        let plain = strip_ansi(&rendered);
        let prompt_line = plain
            .lines()
            .find(|line| line.trim_start().starts_with("> "))
            .expect("startup should render an input line");
        assert_ne!(
            prompt_line.trim(),
            ">",
            "empty input should show a task suggestion:\n{rendered}"
        );
        assert!(
            plain.contains("Getting started"),
            "startup output:\n{rendered}"
        );
        assert!(plain.contains("Workspace"), "startup output:\n{rendered}");
        assert!(
            plain.contains("Delegate agents"),
            "startup output:\n{rendered}"
        );
        assert!(
            plain.contains("Recent activity"),
            "startup output:\n{rendered}"
        );
    }

    #[test]
    fn command_palette_does_not_repeat_the_input_line() {
        let mut out = Vec::new();

        render_chat_home(&mut out, 100, "/", &[], 0, false).unwrap();

        let rendered = String::from_utf8(out).unwrap();
        let plain = strip_ansi(&rendered);
        let prompt_lines = plain.lines().filter(|line| line.trim() == "> /").count();
        assert_eq!(prompt_lines, 1, "command palette output:\n{rendered}");
    }

    #[test]
    fn command_palette_slashes_align_with_the_input() {
        let mut out = Vec::new();

        render_chat_home(&mut out, 100, "/", &[], 0, false).unwrap();

        let rendered = String::from_utf8(out).unwrap();
        let plain = strip_ansi(&rendered);
        let mut palette = plain.lines().skip_while(|line| line.trim_end() != "> /");
        let prompt = palette
            .next()
            .expect("startup should render the slash input");
        let candidate = palette
            .find(|line| line.contains("/agent"))
            .expect("palette should render the first slash command");
        assert_eq!(
            prompt.find('/'),
            candidate.find('/'),
            "input and command columns must align:\n{plain}"
        );
    }

    #[test]
    fn command_palette_scrolls_to_keep_the_selection_visible() {
        let choices = (0..6)
            .map(|index| {
                choice(
                    &format!("worker{index}"),
                    AvailabilityStatus::Available,
                    Some("worker"),
                )
            })
            .collect::<Vec<_>>();
        let selected = matching_commands("/", &choices)
            .iter()
            .position(|item| item.name == "/worker5")
            .expect("test command must be present");

        let mut wide = Vec::new();
        render_chat_home(&mut wide, 100, "/", &choices, selected, false).unwrap();
        let wide = strip_ansi(&String::from_utf8(wide).unwrap());
        assert!(
            wide.lines().any(|line| line.contains("> /worker5")),
            "wide palette must keep the selected command visible:\n{wide}"
        );

        let mut compact = Vec::new();
        render_chat_home(&mut compact, 40, "/", &choices, selected, false).unwrap();
        let compact = strip_ansi(&String::from_utf8(compact).unwrap());
        assert!(
            compact.lines().any(|line| line.contains("> /worker5")),
            "compact palette must keep the selected command visible:\n{compact}"
        );
    }

    #[test]
    fn command_palette_sanitizes_untrusted_agent_labels() {
        let choices = vec![choice(
            "bad\n\x1b[31magent",
            AvailabilityStatus::Available,
            Some("native\n\x1b[2J"),
        )];
        let selected = matching_commands("/", &choices)
            .iter()
            .position(|item| item.name.starts_with("/bad"))
            .unwrap();
        let mut out = Vec::new();

        render_chat_home(&mut out, 100, "/", &choices, selected, false).unwrap();

        let rendered = String::from_utf8(out).unwrap();
        let plain = strip_ansi(&rendered);
        assert!(plain.contains("bad\\n\\u{1b}"));
        assert!(plain.contains("native\\n\\u{1b}[2J"));
        assert!(plain.lines().all(|line| display_width(line) <= 100));
    }

    #[test]
    fn selection_wraps_across_both_edges() {
        assert_eq!(wrapped_selection(0, 3, SelectionDirection::Previous), 2);
        assert_eq!(wrapped_selection(2, 3, SelectionDirection::Next), 0);
        assert_eq!(wrapped_selection(9, 3, SelectionDirection::Next), 0);
        assert_eq!(wrapped_selection(0, 0, SelectionDirection::Previous), 0);
    }

    #[test]
    fn interactive_layout_reserves_the_terminal_last_column() {
        assert_eq!(viewport_content_width(80), 79);
        assert_eq!(viewport_content_width(133), 132);
        assert_eq!(viewport_content_width(200), 132);
    }

    #[test]
    fn interactive_frames_use_synchronized_partial_redraw() {
        let mut out = Vec::new();
        let mut presenter = FramePresenter::default();

        present_chat_home_in_viewport(
            &mut presenter,
            &mut out,
            80,
            usize::MAX,
            "/",
            &[],
            0,
            false,
        )
        .unwrap();

        let rendered = String::from_utf8(out).unwrap();
        assert!(
            rendered.contains("\x1b[?2026h") && rendered.contains("\x1b[?2026l"),
            "frame must be presented as one synchronized terminal update:\n{rendered}"
        );
        assert!(
            !rendered.contains("\x1b[2J"),
            "interactive redraw must not clear the whole terminal:\n{rendered}"
        );
        assert!(
            !rendered.contains("\x1b[J") && !rendered.contains("\x1b[0J"),
            "interactive redraw must not clear the remaining viewport:\n{rendered}"
        );
        assert!(
            !rendered.contains("\n\x1b[?2026l"),
            "a trailing newline can scroll a frame that exactly fills the viewport:\n{rendered}"
        );

        let mut one_row = Vec::new();
        let mut presenter = FramePresenter::default();
        present_chat_home_in_viewport(
            &mut presenter,
            &mut one_row,
            80,
            1,
            "/",
            &[],
            0,
            false,
        )
        .unwrap();
        assert!(
            !String::from_utf8(one_row)
                .unwrap()
                .contains("\n\x1b[?2026l"),
            "one-row frames must not end with a newline"
        );
    }

    #[test]
    fn key_events_require_a_press_kind() {
        assert!(is_press(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        assert!(!is_press(&KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
        assert!(!is_press(&KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )));
    }

    #[test]
    fn terminal_text_escapes_controls_and_counts_cjk_columns() {
        assert_eq!(sanitize_terminal_text("a\n\x1b[31m"), "a\\n\\u{1b}[31m");
        assert_eq!(display_width("数理"), 4);
        assert_eq!(display_width(&truncate("数理逻辑", 5)), 5);
    }

    #[test]
    fn home_input_prefers_self_prompt_and_slash_commands() {
        let choices = vec![choice(
            "codex",
            AvailabilityStatus::Available,
            Some("codex"),
        )];

        assert_eq!(
            parse_home_action("summarize recent commits", &choices),
            HomeAction::Submit("summarize recent commits".into())
        );
        assert_eq!(parse_home_action("/agent", &choices), HomeAction::PickAgent);
        assert_eq!(
            parse_home_action("/codex", &choices),
            HomeAction::LaunchAgent("codex".into())
        );
        assert_eq!(parse_home_action("/quit", &choices), HomeAction::Quit);
        assert_eq!(
            parse_home_action_selected("/", &choices, 5),
            HomeAction::LaunchAgent("codex".into())
        );
        assert_eq!(
            parse_home_action_selected("/cod", &choices, 0),
            HomeAction::LaunchAgent("codex".into())
        );
    }

    #[test]
    fn compact_home_and_help_fit_their_modes() {
        let mut compact = Vec::new();
        render_chat_home(&mut compact, 30, "", &[], 0, false).unwrap();
        let compact = strip_ansi(&String::from_utf8(compact).unwrap());
        assert!(compact.contains("Orchester"));
        assert!(!compact.contains("+----------------"));
        assert!(compact.lines().all(|line| display_width(line) <= 30));

        let mut help = Vec::new();
        render_chat_home(&mut help, 80, "", &[], 0, true).unwrap();
        let help = strip_ansi(&String::from_utf8(help).unwrap());
        assert!(help.contains("/agent      choose a delegate"));
        assert!(help.contains("Esc         close help"));
    }

    #[test]
    fn portrait_home_respects_wide_and_medium_terminal_bounds() {
        let mut wide = Vec::new();
        render_chat_home(&mut wide, 100, "", &[], 0, false).unwrap();
        let wide = strip_ansi(&String::from_utf8(wide).unwrap());
        assert!(wide.lines().all(|line| display_width(line) <= 100));
        assert!(wide.contains('\u{2580}'));

        let mut medium = Vec::new();
        render_chat_home(&mut medium, 80, "", &[], 0, false).unwrap();
        let medium = strip_ansi(&String::from_utf8(medium).unwrap());
        assert!(medium.lines().all(|line| display_width(line) <= 80));
        assert!(medium.chars().filter(|ch| *ch == '\u{2580}').count() > 10);

        let mut narrow = Vec::new();
        render_chat_home(&mut narrow, 55, "", &[], 0, false).unwrap();
        let narrow = strip_ansi(&String::from_utf8(narrow).unwrap());
        assert!(!narrow.contains('\u{2580}'));
    }

    #[test]
    fn chat_home_respects_terminal_height_without_hiding_the_selection() {
        let choices = (0..6)
            .map(|index| {
                choice(
                    &format!("worker{index}"),
                    AvailabilityStatus::Available,
                    Some("worker"),
                )
            })
            .collect::<Vec<_>>();
        let selected = matching_commands("/", &choices)
            .iter()
            .position(|item| item.name == "/worker5")
            .unwrap();

        for (label, input, show_help) in [
            ("empty", "", false),
            ("slash", "/", false),
            ("help", "", true),
            (
                "long task",
                "explain the failing integration test and propose a minimal fix",
                false,
            ),
        ] {
            for height in [12, 24, 30] {
                let mut out = Vec::new();
                render_chat_home_in_viewport(
                    &mut out, 100, height, input, &choices, selected, show_help,
                )
                .unwrap();
                let raw = String::from_utf8(out).unwrap();
                assert!(
                    !raw.contains("\n\x1b[?2026l"),
                    "{label} at {height} rows ended with a scrolling newline"
                );
                let plain = strip_ansi(&raw);
                assert!(
                    plain.lines().count() <= height,
                    "{label} at {height} rows overflowed:\n{plain}"
                );
                assert!(
                    plain.lines().all(|line| display_width(line) <= 100),
                    "{label} at {height} rows exceeded its column budget:\n{plain}"
                );
                assert!(
                    plain
                        .lines()
                        .any(|line| line.trim_start().starts_with("> ")),
                    "{label} input row disappeared at height {height}:\n{plain}"
                );
                assert!(
                    plain.contains("model not configured"),
                    "{label} status row disappeared at height {height}:\n{plain}"
                );
                if label == "slash" {
                    assert!(
                        plain.lines().any(|line| line.contains("> /worker5")),
                        "selected command disappeared at height {height}:\n{plain}"
                    );
                }
                if label == "help" {
                    assert!(
                        plain.contains("/agent      choose a delegate"),
                        "help content disappeared at height {height}:\n{plain}"
                    );
                }
                if height <= 24 {
                    assert!(
                        !plain.lines().any(|line| line.starts_with('+')),
                        "short viewport must use the compact header:\n{plain}"
                    );
                }
            }
        }

        let mut empty = Vec::new();
        render_chat_home_in_viewport(&mut empty, 100, 30, "", &[], 0, false).unwrap();
        let empty = strip_ansi(&String::from_utf8(empty).unwrap());
        assert!(empty.lines().count() <= 30);
        assert_eq!(
            empty.lines().filter(|line| line.starts_with('+')).count(),
            2,
            "30-row empty home should keep both panel borders:\n{empty}"
        );
        assert_eq!(
            empty.lines().filter(|line| line.starts_with('|')).count(),
            avatar::HEIGHT,
            "30-row empty home should keep every portrait row:\n{empty}"
        );
        assert!(
            empty.contains('\u{2580}'),
            "30-row empty home should keep portrait"
        );
    }

    fn strip_ansi(input: &str) -> String {
        let mut plain = String::new();
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                if matches!(chars.next(), Some('[')) {
                    for end in chars.by_ref() {
                        if ('@'..='~').contains(&end) {
                            break;
                        }
                    }
                }
            } else {
                plain.push(ch);
            }
        }
        plain
    }

    fn choice(name: &str, status: AvailabilityStatus, command: Option<&str>) -> AgentChoice {
        AgentChoice {
            name: name.into(),
            kinds: "code,chat".into(),
            supports_resume: true,
            status,
            detail: "test".into(),
            native_command: command.map(str::to_string),
        }
    }
}
