use super::AgentChoice;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptAction {
    Run(String),
    PickAgent,
    LaunchAgent(String),
    ListAgents,
    Workspace(WorkspaceCommand),
    Plugins(PluginAction),
    Help,
    Quit,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeAction {
    Submit(String),
    PickAgent,
    LaunchAgent(String),
    Workspace(WorkspaceCommand),
    Plugins(PluginAction),
    Help,
    Quit,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginAction {
    List,
    Status(String),
    Install(String),
    Remove(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceCommand {
    Status,
    Config,
    Permissions,
    Resume,
    Model(ModelCommand),
    Theme(ThemeCommand),
    Credential(CredentialCommand),
}

/// Provider credential entry.  `provider: None` means "whichever provider the
/// effective config marks active", so the common case needs no argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialCommand {
    Login { provider: Option<String> },
    Logout { provider: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelCommand {
    Show,
    SelectProfile(String),
    UseConfigured,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeCommand {
    Show,
    Select(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommandAction {
    PickAgent,
    ListAgents,
    Workspace(WorkspaceCommand),
    Plugins,
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
        PromptAction::Workspace(command) => HomeAction::Workspace(command),
        PromptAction::Plugins(action) => HomeAction::Plugins(action),
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
    let mut matches = command_items(choices)
        .into_iter()
        .filter(|item| {
            let name = item.name.trim_start_matches('/').to_ascii_lowercase();
            normalized.is_empty()
                || name.starts_with(&normalized)
                || item.description.to_ascii_lowercase().contains(&normalized)
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|item| {
        !item
            .name
            .trim_start_matches('/')
            .to_ascii_lowercase()
            .starts_with(&normalized)
    });
    matches
}

pub(super) fn matching_delegate_commands(query: &str, choices: &[AgentChoice]) -> Vec<CommandItem> {
    matching_commands(query, choices)
        .into_iter()
        .filter(|item| item.action != CommandAction::Plugins)
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
        "/status" => {
            return if input.split_whitespace().count() == 1 {
                PromptAction::Workspace(WorkspaceCommand::Status)
            } else {
                PromptAction::Help
            };
        }
        // Matched on the token before the palette fallback: `/model` is described
        // as "show configured self-agent models", so a description substring match
        // would otherwise route `/config` there.
        "/config" => {
            return if input.split_whitespace().count() == 1 {
                PromptAction::Workspace(WorkspaceCommand::Config)
            } else {
                PromptAction::Help
            };
        }
        "/permissions" | "/permission" => {
            return if input.split_whitespace().count() == 1 {
                PromptAction::Workspace(WorkspaceCommand::Permissions)
            } else {
                PromptAction::Help
            };
        }
        "/resume" => {
            return if input.split_whitespace().count() == 1 {
                PromptAction::Workspace(WorkspaceCommand::Resume)
            } else {
                PromptAction::Help
            };
        }
        "/model" => {
            return parse_model_command(input)
                .map(|command| PromptAction::Workspace(WorkspaceCommand::Model(command)))
                .unwrap_or(PromptAction::Help);
        }
        "/theme" => {
            return parse_theme_command(input)
                .map(|command| PromptAction::Workspace(WorkspaceCommand::Theme(command)))
                .unwrap_or(PromptAction::Help);
        }
        "/login" => {
            return provider_argument(input)
                .map(|provider| {
                    PromptAction::Workspace(WorkspaceCommand::Credential(
                        CredentialCommand::Login { provider },
                    ))
                })
                .unwrap_or(PromptAction::Help);
        }
        "/logout" => {
            return provider_argument(input)
                .map(|provider| {
                    PromptAction::Workspace(WorkspaceCommand::Credential(
                        CredentialCommand::Logout { provider },
                    ))
                })
                .unwrap_or(PromptAction::Help);
        }
        "/plugin" | "/plugins" => {
            return parse_plugin_action(input)
                .map(PromptAction::Plugins)
                .unwrap_or(PromptAction::Help);
        }
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
    match item.action.clone() {
        CommandAction::PickAgent => PromptAction::PickAgent,
        CommandAction::ListAgents => PromptAction::ListAgents,
        CommandAction::Workspace(command) => PromptAction::Workspace(command),
        CommandAction::Plugins => PromptAction::Plugins(PluginAction::List),
        CommandAction::Help => PromptAction::Help,
        CommandAction::Quit => PromptAction::Quit,
        CommandAction::LaunchAgent => item
            .agent
            .clone()
            .map(PromptAction::LaunchAgent)
            .unwrap_or(PromptAction::Empty),
    }
}

/// Accept `/<command>` or `/<command> <provider>` and reject anything longer.
/// `Some(None)` means "no provider named"; `None` means the input is malformed.
fn provider_argument(input: &str) -> Option<Option<String>> {
    let mut parts = input.split_whitespace();
    parts.next()?;
    let provider = parts.next().map(str::to_owned);
    if parts.next().is_some() {
        return None;
    }
    Some(provider)
}

fn parse_model_command(input: &str) -> Option<ModelCommand> {
    let mut parts = input.split_whitespace();
    parts.next()?;
    let selection = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match selection {
        None => Some(ModelCommand::Show),
        Some("configured" | "--configured") => Some(ModelCommand::UseConfigured),
        Some(name) => Some(ModelCommand::SelectProfile(name.to_owned())),
    }
}

fn parse_theme_command(input: &str) -> Option<ThemeCommand> {
    let mut parts = input.split_whitespace();
    parts.next()?;
    let selection = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match selection {
        None => Some(ThemeCommand::Show),
        Some(name) => Some(ThemeCommand::Select(name.to_ascii_lowercase())),
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
            name: "/status".into(),
            description: "show self-agent workspace status".into(),
            action: CommandAction::Workspace(WorkspaceCommand::Status),
            agent: None,
        },
        CommandItem {
            name: "/config".into(),
            description: "show resolved self-agent configuration".into(),
            action: CommandAction::Workspace(WorkspaceCommand::Config),
            agent: None,
        },
        CommandItem {
            name: "/permissions".into(),
            description: "show effective self-agent permissions".into(),
            action: CommandAction::Workspace(WorkspaceCommand::Permissions),
            agent: None,
        },
        CommandItem {
            name: "/resume".into(),
            description: "show resumable self-agent runs".into(),
            action: CommandAction::Workspace(WorkspaceCommand::Resume),
            agent: None,
        },
        CommandItem {
            name: "/model".into(),
            description: "show configured self-agent models".into(),
            action: CommandAction::Workspace(WorkspaceCommand::Model(ModelCommand::Show)),
            agent: None,
        },
        CommandItem {
            name: "/theme".into(),
            description: "choose the terminal theme".into(),
            action: CommandAction::Workspace(WorkspaceCommand::Theme(ThemeCommand::Show)),
            agent: None,
        },
        CommandItem {
            name: "/login".into(),
            description: "store a provider API key".into(),
            action: CommandAction::Workspace(WorkspaceCommand::Credential(
                CredentialCommand::Login { provider: None },
            )),
            agent: None,
        },
        CommandItem {
            name: "/logout".into(),
            description: "forget a stored provider API key".into(),
            action: CommandAction::Workspace(WorkspaceCommand::Credential(
                CredentialCommand::Logout { provider: None },
            )),
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
    items.push(CommandItem {
        name: "/plugins".into(),
        description: "manage validated agent plugins".into(),
        action: CommandAction::Plugins,
        agent: None,
    });
    items
}

fn parse_plugin_action(input: &str) -> Option<PluginAction> {
    let mut parts = input.split_whitespace();
    parts.next()?;
    let operation = parts.next().unwrap_or("list").to_ascii_lowercase();
    let name = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match (operation.as_str(), name) {
        ("list", None) => Some(PluginAction::List),
        ("status", Some(name)) => Some(PluginAction::Status(name.to_owned())),
        ("install", Some(name)) => Some(PluginAction::Install(name.to_owned())),
        ("remove", Some(name)) => Some(PluginAction::Remove(name.to_owned())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_command_opens_picker_or_selects_named_theme() {
        assert_eq!(
            parse_prompt_action("/theme", &[]),
            PromptAction::Workspace(WorkspaceCommand::Theme(ThemeCommand::Show))
        );
        assert_eq!(
            parse_home_action("/theme light", &[]),
            HomeAction::Workspace(WorkspaceCommand::Theme(ThemeCommand::Select(
                "light".into()
            )))
        );
        assert_eq!(
            parse_prompt_action("/theme light extra", &[]),
            PromptAction::Help
        );
    }

    #[test]
    fn command_palette_contains_theme_picker() {
        let theme = command_items(&[])
            .into_iter()
            .find(|item| item.name == "/theme")
            .expect("theme command");

        assert_eq!(theme.description, "choose the terminal theme");
        assert_eq!(
            command_action("/", Some(&theme)),
            PromptAction::Workspace(WorkspaceCommand::Theme(ThemeCommand::Show))
        );
    }

    #[test]
    fn partial_command_selection_confirms_the_highlighted_item() {
        assert_eq!(
            parse_home_action_selected("/sta", &[], 0),
            HomeAction::Workspace(WorkspaceCommand::Status)
        );
        assert_eq!(
            parse_home_action_selected("/mod", &[], 0),
            HomeAction::Workspace(WorkspaceCommand::Model(ModelCommand::Show))
        );
        assert_eq!(
            parse_home_action_selected("/res", &[], 0),
            HomeAction::Workspace(WorkspaceCommand::Resume)
        );
    }

    #[test]
    fn command_name_prefixes_rank_before_description_matches() {
        let status = matching_commands("/sta", &[]);
        assert_eq!(
            status.first().map(|item| item.name.as_str()),
            Some("/status")
        );
        assert!(status.iter().any(|item| item.name == "/list"));

        let resume = matching_commands("/res", &[]);
        assert_eq!(
            resume.first().map(|item| item.name.as_str()),
            Some("/resume")
        );
        assert!(resume.iter().any(|item| item.name == "/config"));
    }

    #[test]
    fn exact_commands_with_invalid_arguments_still_show_help() {
        for input in [
            "/status extra",
            "/resume extra",
            "/model fast extra",
            "/theme light extra",
            "/plugins list extra",
        ] {
            assert_eq!(
                parse_home_action_selected(input, &[], 0),
                HomeAction::Help,
                "{input} must not fall back to the highlighted palette item"
            );
        }
    }
}
