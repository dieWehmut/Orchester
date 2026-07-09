use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, Write};

use crossterm::cursor;
use crossterm::event::{self, Event as TerminalEvent, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, ClearType};
use orchester_protokoll::{Capability, TaskKind};
use orchester_vertrag::{AdapterAvailability, AvailabilityStatus};
use orchester_verzeichnis::Registry;

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const ORANGE: &str = "\x1b[38;5;208m";
const PINK: &str = "\x1b[38;5;218m";
const LAVENDER: &str = "\x1b[38;5;147m";
const INK: &str = "\x1b[38;5;245m";
const RESET: &str = "\x1b[0m";

// Generated from D:\picture\dieWehmut-2.jpg as terminal character art so the
// installed binary can render the portrait without reading the image file.
const AVATAR_WIDTH: usize = 34;
const AVATAR_LINES: &[&str] = &[
    "%%=   .  :::-::::.:.::. .:. -#****",
    "%%*  .. ...::.-::::::::..:. :*#***",
    "%%%: .. ..:::::-::::-::::. . .+###",
    "%%%+   ..-::----=------::.    :+#%",
    "%%%#. ..:-:--======--=--.:..  :++#",
    "%%%*. ..----=-====+==-=-:.:.. .*#*",
    "%%#+ ..:---==-=++++++===-::.. -=*#",
    "@%#- .::-===-+-=+++++==+-::....*-*",
    "%##...:-==+==+=-=++=*+==-::..: .-",
    "##+. .--=++=**+===+==++==::..:. ..",
    "*#-. :--++==%#**#***+====-...::",
    "#+   ---++=*@#%**%###+::--:.:::..",
    "+.=..-=-++=#@##%**@%-..=:--=:::...",
    ":+* :-=-++=%%%#%@##%+=++==*#-.:..:",
    "*%=.--=-+++=::=#%@%%%****=*#::.. +",
    "*%:.::==++:+-==%%%%%%%%##=+=:-: .+",
    "#*.:.:==++=#*+#%%%%%%%%%*--.--+..-",
    "%-.-::=:=+*%##%%%%%%%%%%+--::-+  +",
    "* :#-:=:-++%%%%%%%%@%%%#+:=-::. .*",
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

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), cursor::Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), cursor::Show);
    }
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

    let _guard = TerminalGuard::enter()?;
    let mut out = io::stdout();
    let mut selected = default_index(&selectable, default_agent);
    let mut command = String::new();
    let mut command_selected = 0usize;
    let mut message = String::new();

    loop {
        render_home(
            &mut out,
            choices,
            selected,
            &command,
            command_selected,
            &message,
        )?;

        let TerminalEvent::Key(key) = event::read()? else {
            continue;
        };
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            clear_screen(&mut out)?;
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
                    command_selected = command_selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    if command_selected + 1 < matches.len() {
                        command_selected += 1;
                    }
                }
                KeyCode::Enter => {
                    let action = command_action(&command, matches.get(command_selected));
                    match action {
                        PromptAction::Quit => {
                            clear_screen(&mut out)?;
                            return Ok(None);
                        }
                        PromptAction::LaunchAgent(name) => {
                            if let Some(agent) = choices.iter().find(|choice| choice.name == name) {
                                clear_screen(&mut out)?;
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
                clear_screen(&mut out)?;
                return Ok(None);
            }
            KeyCode::Char('/') => {
                command.push('/');
                command_selected = 0;
                message.clear();
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                clear_screen(&mut out)?;
                return Ok(None);
            }
            KeyCode::Up => {
                selected = selected.saturating_sub(1);
                message.clear();
            }
            KeyCode::Down => {
                if selected + 1 < selectable.len() {
                    selected += 1;
                }
                message.clear();
            }
            KeyCode::Enter => {
                clear_screen(&mut out)?;
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

    render_welcome(out)?;
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

fn render_home<W: Write>(
    out: &mut W,
    choices: &[AgentChoice],
    selected: usize,
    command: &str,
    command_selected: usize,
    message: &str,
) -> io::Result<()> {
    clear_screen(out)?;
    let selectable = selectable_agents(choices);
    let selected_agent = selectable.get(selected);
    let (cols, _) = terminal::size().unwrap_or((100, 30));
    let width = (cols as usize).clamp(50, 132);

    writeln!(
        out,
        "{ORANGE}{BOLD}Orchester{RESET} {DIM}v{}  local agent conductor{RESET}",
        env!("CARGO_PKG_VERSION")
    )?;
    render_home_panel(out, width, selected_agent)?;
    writeln!(out)?;

    writeln!(out, "{BOLD}Choose agent{RESET}")?;
    for (i, choice) in selectable.iter().enumerate() {
        let pointer = if i == selected { ">" } else { " " };
        let row_color = if i == selected { CYAN } else { "" };
        let row_reset = if i == selected { RESET } else { "" };
        writeln!(
            out,
            " {row_color}{pointer} {name:<10}{row_reset} {status:<8} {kinds:<18} {launch}",
            name = choice.name,
            status = plain_status(choice.status),
            kinds = choice.kinds,
            launch = launch_label(choice)
        )?;
        if i == selected {
            writeln!(out, "    {DIM}{}{RESET}", choice.detail)?;
        }
    }

    let unavailable = choices
        .iter()
        .filter(|choice| !choice.is_available())
        .collect::<Vec<_>>();
    if !unavailable.is_empty() {
        writeln!(out)?;
        writeln!(
            out,
            "{DIM}Unavailable: {}{RESET}",
            unavailable_names(&unavailable)
        )?;
    }

    writeln!(out)?;
    if command.starts_with('/') {
        render_command_palette(out, command, choices, command_selected)?;
    } else if !message.is_empty() {
        writeln!(out, "{YELLOW}{message}{RESET}")?;
    } else {
        writeln!(
            out,
            "{DIM}Type / to search commands. Press q or Esc to exit.{RESET}"
        )?;
    }
    out.flush()
}

fn render_home_panel<W: Write>(
    out: &mut W,
    width: usize,
    selected_agent: Option<&AgentChoice>,
) -> io::Result<()> {
    let content_width = width.saturating_sub(7);
    let left_width = AVATAR_WIDTH
        .min(content_width / 2 + 4)
        .min(content_width.saturating_sub(18))
        .max(12);
    let right_width = content_width.saturating_sub(left_width);
    let selected = selected_agent
        .map(|agent| format!("{} ({})", agent.name, launch_label(agent)))
        .unwrap_or_else(|| "none".into());
    let cwd = std::env::current_dir()
        .map(|cwd| cwd.display().to_string())
        .unwrap_or_else(|_| ".".into());
    let right_rows = [
        "Welcome back".to_string(),
        String::new(),
        "Tips for getting started".to_string(),
        "  Enter launches the highlighted CLI".to_string(),
        "  / opens matching commands".to_string(),
        String::new(),
        "Agent choice".to_string(),
        "  Choose on every Orchester launch".to_string(),
        format!("  Selected: {selected}"),
        String::new(),
        "Commands".to_string(),
        "  /agent  /list  /help  /quit".to_string(),
        String::new(),
        format!("cwd {cwd}"),
    ];

    writeln!(out, "{ORANGE}+{}+{RESET}", "-".repeat(width - 2))?;
    let rows = AVATAR_LINES.len().max(right_rows.len());
    for row in 0..rows {
        let avatar = AVATAR_LINES.get(row).copied().unwrap_or("");
        let avatar = truncate(avatar, left_width);
        let avatar_pad = " ".repeat(left_width.saturating_sub(avatar.chars().count()));
        let right = right_rows.get(row).map(String::as_str).unwrap_or("");
        let right = truncate(right, right_width);

        write!(out, "{ORANGE}|{RESET} ")?;
        write_avatar_line(out, &avatar)?;
        write!(out, "{avatar_pad} {ORANGE}|{RESET} ")?;
        write!(out, "{right:<right_width$}")?;
        writeln!(out, " {ORANGE}|{RESET}")?;
    }
    writeln!(out, "{ORANGE}+{}+{RESET}", "-".repeat(width - 2))
}

fn write_avatar_line<W: Write>(out: &mut W, line: &str) -> io::Result<()> {
    for ch in line.chars() {
        if ch == ' ' {
            write!(out, " ")?;
        } else {
            write!(out, "{}{ch}{RESET}", avatar_color(ch))?;
        }
    }
    Ok(())
}

fn avatar_color(ch: char) -> &'static str {
    match ch {
        '@' | '%' | '#' => INK,
        '*' | '+' | '=' => PINK,
        '-' | ':' => LAVENDER,
        _ => DIM,
    }
}

fn render_command_palette<W: Write>(
    out: &mut W,
    command: &str,
    choices: &[AgentChoice],
    selected: usize,
) -> io::Result<()> {
    writeln!(out, "{BOLD}> {command}{RESET}")?;
    let matches = matching_commands(command, choices);
    if matches.is_empty() {
        writeln!(out, "  {DIM}No matching commands{RESET}")?;
        return Ok(());
    }
    for (i, item) in matches.iter().take(8).enumerate() {
        let marker = if i == selected { ">" } else { " " };
        let color = if i == selected { ORANGE } else { "" };
        let reset = if i == selected { RESET } else { "" };
        writeln!(
            out,
            "  {color}{marker} {:<16}{reset} {DIM}{}{RESET}",
            item.name, item.description
        )?;
    }
    Ok(())
}

fn render_welcome<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "{BOLD}{CYAN}Orchester{RESET} {DIM}local agent conductor{RESET}"
    )?;
    writeln!(
        out,
        "{DIM}Pick an installed agent, then type a task prompt.{RESET}"
    )?;
    writeln!(out)
}

fn render_no_runnable_agents<W: Write>(out: &mut W) -> io::Result<()> {
    writeln!(out, "{RED}No runnable agents were found.{RESET}")
}

fn clear_screen<W: Write>(out: &mut W) -> io::Result<()> {
    execute!(out, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))
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

fn unavailable_names(choices: &[&AgentChoice]) -> String {
    choices
        .iter()
        .map(|choice| choice.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn truncate(s: &str, max: usize) -> String {
    let chars = s.chars().collect::<Vec<_>>();
    if chars.len() <= max {
        return s.to_string();
    }
    if max < 3 {
        return chars.into_iter().take(max).collect();
    }
    chars.into_iter().take(max - 3).collect::<String>() + "..."
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
    fn home_renders_avatar_and_explicit_choice_copy() {
        let choices = vec![choice(
            "codex",
            AvailabilityStatus::Available,
            Some("codex"),
        )];
        let mut out = Vec::new();

        render_home(&mut out, &choices, 0, "", 0, "").unwrap();

        let rendered = String::from_utf8_lossy(&out);
        let plain = strip_ansi(&rendered);
        assert!(plain.contains("%%="), "home output:\n{rendered}");
        assert!(
            rendered.contains("Choose on every Orchester launch"),
            "home output:\n{rendered}"
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
