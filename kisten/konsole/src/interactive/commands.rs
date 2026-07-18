use super::AgentChoice;

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
pub(super) struct CommandItem {
    pub(super) name: String,
    pub(super) description: String,
    action: CommandAction,
    agent: Option<String>,
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

pub(super) fn parse_home_action_selected(
    input: &str,
    choices: &[AgentChoice],
    selected: usize,
) -> HomeAction {
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

pub(super) fn matching_commands(query: &str, choices: &[AgentChoice]) -> Vec<CommandItem> {
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

pub(super) fn command_action(input: &str, selected: Option<&CommandItem>) -> PromptAction {
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
