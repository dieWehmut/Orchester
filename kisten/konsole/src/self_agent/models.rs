use std::io::{self, Write};

use orchester_laufzeit::harness::service::{
    SelfAgentActiveModel, SelfAgentModelCatalog, SelfAgentModelChoice, SelfAgentProviderState,
};

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn render_models(out: &mut impl Write, catalog: &SelfAgentModelCatalog) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{BOLD}Self-agent models{RESET}")?;
    match &catalog.active {
        SelfAgentActiveModel::Configured(active) => {
            let selection = active.profile.as_deref().unwrap_or("configured");
            writeln!(out, "active: {}", safe_metadata(selection))?;
            render_choice(out, active, "  ")?;
        }
        SelfAgentActiveModel::Unresolved { path, message } => {
            writeln!(
                out,
                "active: unresolved ({}: {})",
                safe_metadata(path),
                safe_metadata(message)
            )?;
        }
        SelfAgentActiveModel::NotConfigured => writeln!(out, "active: not configured")?,
    }

    writeln!(out, "profiles:")?;
    if catalog.profiles.is_empty() {
        writeln!(out, "  none configured")?;
    } else {
        for choice in &catalog.profiles {
            let name = choice.profile.as_deref().unwrap_or("unnamed");
            writeln!(out, "  {}", safe_metadata(name))?;
            render_choice(out, choice, "    ")?;
        }
    }

    writeln!(out, "providers:")?;
    if catalog.providers.is_empty() {
        writeln!(out, "  none configured")?;
    } else {
        for provider in &catalog.providers {
            let selected = catalog.selected_provider.as_deref() == Some(provider.provider.as_str());
            writeln!(
                out,
                "  {}{}",
                safe_metadata(&provider.provider),
                if selected { " (selected)" } else { "" }
            )?;
            match &provider.state {
                SelfAgentProviderState::Selectable { model, wire_api } => writeln!(
                    out,
                    "    model: {} | provider: {} | wire: {}",
                    safe_metadata(model),
                    safe_metadata(&provider.provider_name),
                    safe_metadata(wire_api)
                )?,
                // The field to repair is the whole point of listing an entry
                // that cannot be selected.
                SelfAgentProviderState::Unavailable { path, message } => writeln!(
                    out,
                    "    unavailable: {} ({})",
                    safe_metadata(path),
                    safe_metadata(message)
                )?,
            }
        }
    }
    writeln!(out, "{DIM}No provider request was made.{RESET}")?;
    writeln!(out)
}

pub fn render_model_selection(
    out: &mut impl Write,
    choice: &SelfAgentModelChoice,
) -> io::Result<()> {
    let selection = choice.profile.as_deref().unwrap_or("configured");
    writeln!(out)?;
    writeln!(
        out,
        "{BOLD}Model selected{RESET}: {}",
        safe_metadata(selection)
    )?;
    render_choice(out, choice, "  ")?;
    writeln!(
        out,
        "{DIM}Applies to future turns in this session; configuration was not changed.{RESET}"
    )?;
    writeln!(out)
}

fn render_choice(
    out: &mut impl Write,
    choice: &SelfAgentModelChoice,
    indent: &str,
) -> io::Result<()> {
    writeln!(
        out,
        "{indent}model: {} | provider: {} ({})",
        safe_metadata(&choice.model),
        safe_metadata(&choice.provider_name),
        safe_metadata(&choice.provider)
    )?;
    writeln!(
        out,
        "{indent}reasoning: {} | plan: {} | tier: {}",
        optional_value(choice.reasoning_effort.as_deref()),
        optional_value(choice.plan_reasoning_effort.as_deref()),
        optional_value(choice.service_tier.as_deref())
    )
}

fn optional_value(value: Option<&str>) -> String {
    value.map(safe_metadata).unwrap_or_else(|| "default".into())
}

/// Escape every control character, newlines included.  Metadata is rendered on
/// a single line, so a newline inside it could otherwise forge one.
pub(super) fn safe_metadata(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if character.is_control() {
                character.escape_default().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchester_laufzeit::harness::service::SelfAgentProviderChoice;

    #[test]
    fn rendering_sanitizes_catalog_metadata() {
        let catalog = catalog(
            SelfAgentActiveModel::Configured(choice(None, "gpt-default\x1b[31m")),
            vec![choice(Some("review\nprofile"), "gpt-review")],
        );
        let mut output = Vec::new();

        render_models(&mut output, &catalog).expect("render model catalog");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("Self-agent models"));
        assert!(rendered.contains("gpt-default\\u{1b}[31m"));
        assert!(rendered.contains("review\\nprofile"));
        assert!(!rendered.contains("gpt-default\x1b[31m"));
        assert!(!rendered.contains("review\nprofile"));
    }

    #[test]
    fn an_unresolved_active_model_still_lists_selectable_profiles() {
        let catalog = catalog(
            SelfAgentActiveModel::Unresolved {
                path: "model_provider".into(),
                message: "active model provider is not configured".into(),
            },
            vec![choice(Some("fast"), "gpt-fast")],
        );
        let mut output = Vec::new();

        render_models(&mut output, &catalog).expect("render model catalog");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("active: unresolved (model_provider:"));
        assert!(rendered.contains("active model provider is not configured"));
        // A broken active model must not read as an absent one, and it must
        // not hide the profiles that would repair it.
        assert!(!rendered.contains("active: not configured"));
        assert!(rendered.contains("fast"));
        assert!(rendered.contains("gpt-fast"));
    }

    #[test]
    fn selection_rendering_identifies_session_scope() {
        let selected = choice(Some("fast"), "gpt-fast");
        let mut output = Vec::new();

        render_model_selection(&mut output, &selected).expect("render selected model");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("Model selected"));
        assert!(rendered.contains("fast"));
        assert!(rendered.contains("gpt-fast"));
        assert!(rendered.contains("future turns in this session"));
        assert!(rendered.contains("configuration was not changed"));
    }

    #[test]
    fn the_provider_listing_names_the_wire_and_the_field_to_repair() {
        let mut catalog = catalog(
            SelfAgentActiveModel::Configured(choice(None, "gpt-default")),
            Vec::new(),
        );
        catalog.providers = vec![
            SelfAgentProviderChoice {
                provider: "relay".into(),
                provider_name: "relay API".into(),
                state: SelfAgentProviderState::Selectable {
                    model: "gpt-default".into(),
                    wire_api: "anthropic".into(),
                },
            },
            SelfAgentProviderChoice {
                provider: "broken".into(),
                provider_name: "broken".into(),
                state: SelfAgentProviderState::Unavailable {
                    path: "model_providers.broken.base_url".into(),
                    message: "provider base URL is not configured".into(),
                },
            },
        ];
        catalog.selected_provider = Some("relay".into());
        let mut output = Vec::new();

        render_models(&mut output, &catalog).expect("render model catalog");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("providers:"));
        assert!(rendered.contains("relay (selected)"));
        assert!(rendered.contains("wire: anthropic"));
        assert!(rendered.contains("unavailable: model_providers.broken.base_url"));
        assert!(rendered.contains("provider base URL is not configured"));
    }

    #[test]
    fn an_empty_provider_block_reads_as_absent_rather_than_broken() {
        let catalog = catalog(SelfAgentActiveModel::NotConfigured, Vec::new());
        let mut output = Vec::new();

        render_models(&mut output, &catalog).expect("render model catalog");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert_eq!(rendered.matches("none configured").count(), 2);
    }

    fn catalog(
        active: SelfAgentActiveModel,
        profiles: Vec<SelfAgentModelChoice>,
    ) -> SelfAgentModelCatalog {
        SelfAgentModelCatalog {
            active,
            profiles,
            providers: Vec::new(),
            selected_provider: None,
        }
    }

    fn choice(profile: Option<&str>, model: &str) -> SelfAgentModelChoice {
        SelfAgentModelChoice {
            profile: profile.map(str::to_owned),
            provider: "OpenAI".into(),
            provider_name: "OpenAI API".into(),
            model: model.into(),
            reasoning_effort: Some("high".into()),
            plan_reasoning_effort: None,
            service_tier: Some("default".into()),
        }
    }
}
