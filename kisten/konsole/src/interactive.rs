use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, Write};

use orchester_protokoll::{Capability, TaskKind};
use orchester_vertrag::{AdapterAvailability, AvailabilityStatus};
use orchester_verzeichnis::Registry;

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentChoice {
    pub name: String,
    pub kinds: String,
    pub supports_resume: bool,
    pub status: AvailabilityStatus,
    pub detail: String,
}

impl AgentChoice {
    pub fn is_selectable(&self) -> bool {
        self.status != AvailabilityStatus::Missing
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptAction {
    Run(String),
    PickAgent,
    ListAgents,
    Help,
    Quit,
    Empty,
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
            }
        })
        .collect::<Vec<_>>();

    choices.sort_by(|a, b| {
        status_rank(a.status)
            .cmp(&status_rank(b.status))
            .then_with(|| agent_rank(&a.name).cmp(&agent_rank(&b.name)))
            .then_with(|| a.name.cmp(&b.name))
    });
    choices
}

pub fn select_agent<R: BufRead, W: Write>(
    input: &mut R,
    out: &mut W,
    choices: &[AgentChoice],
    default_agent: Option<&str>,
) -> io::Result<Option<AgentChoice>> {
    let selectable = choices
        .iter()
        .filter(|choice| choice.is_selectable())
        .collect::<Vec<_>>();
    if selectable.is_empty() {
        writeln!(out, "{RED}No runnable agents were found.{RESET}")?;
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
            return Ok(Some((*selectable[0]).clone()));
        }
        if is_quit(trimmed) {
            writeln!(out)?;
            return Ok(None);
        }

        if let Ok(n) = trimmed.parse::<usize>() {
            if (1..=selectable.len()).contains(&n) {
                writeln!(out)?;
                return Ok(Some((*selectable[n - 1]).clone()));
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
) -> io::Result<PromptAction> {
    write!(out, "{CYAN}{}>{RESET} ", agent.name)?;
    if resume.is_some() {
        write!(out, "{DIM}resume{RESET} ")?;
    }
    out.flush()?;

    let Some(line) = read_line(input)? else {
        return Ok(PromptAction::Quit);
    };
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(PromptAction::Empty);
    }

    Ok(match trimmed {
        "/agent" | "/a" => PromptAction::PickAgent,
        "/list" | "/agents" | "/l" => PromptAction::ListAgents,
        "/help" | "/h" | "?" => PromptAction::Help,
        "/quit" | "/exit" | "/q" => PromptAction::Quit,
        _ => PromptAction::Run(trimmed.to_string()),
    })
}

pub fn render_agent_table<W: Write>(
    out: &mut W,
    choices: &[AgentChoice],
    default_agent: Option<&str>,
) -> io::Result<()> {
    let selectable = choices
        .iter()
        .filter(|choice| choice.is_selectable())
        .collect::<Vec<_>>();

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
            resume_label(choice.supports_resume),
            default
        )?;
        writeln!(out, "      {DIM}{}{RESET}", choice.detail)?;
    }

    let missing = choices
        .iter()
        .filter(|choice| !choice.is_selectable())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        writeln!(out)?;
        writeln!(out, "{BOLD}Not available on this PATH{RESET}")?;
        for choice in missing {
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
    writeln!(out, "  text     send a task to the selected agent")?;
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

fn read_line<R: BufRead>(input: &mut R) -> io::Result<Option<String>> {
    let mut line = String::new();
    let read = input.read_line(&mut line)?;
    if read == 0 {
        Ok(None)
    } else {
        Ok(Some(line))
    }
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

fn resume_label(supports_resume: bool) -> &'static str {
    if supports_resume {
        "resume"
    } else {
        "no-resume"
    }
}

fn status_rank(status: AvailabilityStatus) -> u8 {
    match status {
        AvailabilityStatus::Available => 0,
        AvailabilityStatus::Unknown => 1,
        AvailabilityStatus::Missing => 2,
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
            choice("codex", AvailabilityStatus::Missing),
            choice("mock", AvailabilityStatus::Available),
        ];
        let mut input = Cursor::new(b"mock\n");
        let mut out = Vec::new();

        let selected = select_agent(&mut input, &mut out, &choices, None)
            .unwrap()
            .unwrap();

        assert_eq!(selected.name, "mock");
    }

    #[test]
    fn select_agent_skips_missing_numbering() {
        let choices = vec![
            choice("codex", AvailabilityStatus::Missing),
            choice("mock", AvailabilityStatus::Available),
        ];
        let mut input = Cursor::new(b"1\n");
        let mut out = Vec::new();

        let selected = select_agent(&mut input, &mut out, &choices, None)
            .unwrap()
            .unwrap();

        assert_eq!(selected.name, "mock");
    }

    #[test]
    fn prompt_action_parses_commands() {
        let agent = choice("mock", AvailabilityStatus::Available);
        let mut input = Cursor::new(b"/agent\n");
        let mut out = Vec::new();

        let action = read_prompt_action(&mut input, &mut out, &agent, None).unwrap();

        assert_eq!(action, PromptAction::PickAgent);
    }

    #[test]
    fn prompt_action_runs_plain_text() {
        let agent = choice("mock", AvailabilityStatus::Available);
        let mut input = Cursor::new(b"write tests\n");
        let mut out = Vec::new();

        let action = read_prompt_action(&mut input, &mut out, &agent, Some("sid")).unwrap();

        assert_eq!(action, PromptAction::Run("write tests".into()));
    }

    fn choice(name: &str, status: AvailabilityStatus) -> AgentChoice {
        AgentChoice {
            name: name.into(),
            kinds: "code,chat".into(),
            supports_resume: true,
            status,
            detail: "test".into(),
        }
    }
}
