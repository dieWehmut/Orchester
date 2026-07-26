use std::io::{self, Write};

use orchester_laufzeit::harness::service::{SelfAgentModelCatalog, SelfAgentModelChoice};

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn render_models(out: &mut impl Write, catalog: &SelfAgentModelCatalog) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{BOLD}Self-agent models{RESET}")?;
    match catalog.configured.as_ref() {
        Some(active) => {
            let selection = active.profile.as_deref().unwrap_or("configured");
            writeln!(out, "active: {}", safe_metadata(selection))?;
            render_choice(out, active, "  ")?;
        }
        None => writeln!(out, "active: not configured")?,
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
    writeln!(out, "{DIM}No provider request was made.{RESET}")?;
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

fn safe_metadata(value: &str) -> String {
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

    #[test]
    fn rendering_sanitizes_catalog_metadata() {
        let catalog = SelfAgentModelCatalog {
            configured: Some(choice(None, "gpt-default\x1b[31m")),
            profiles: vec![choice(Some("review\nprofile"), "gpt-review")],
        };
        let mut output = Vec::new();

        render_models(&mut output, &catalog).expect("render model catalog");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("Self-agent models"));
        assert!(rendered.contains("gpt-default\\u{1b}[31m"));
        assert!(rendered.contains("review\\nprofile"));
        assert!(!rendered.contains("gpt-default\x1b[31m"));
        assert!(!rendered.contains("review\nprofile"));
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
