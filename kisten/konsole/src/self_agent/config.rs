use std::io::{self, Write};

use orchester_laufzeit::harness::service::{ConfigResolution, SelfAgentConfigView};

use super::render::safe_terminal_text;

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn render_config(out: &mut impl Write, view: &SelfAgentConfigView) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{BOLD}Self-agent configuration{RESET}")?;
    // Both layers are named whether or not they exist: "absent" answers "why is
    // my setting ignored" as often as a wrong value does.
    render_layer(out, "user", &view.user_path, view.user_present)?;
    render_layer(out, "project", &view.project_path, view.project_present)?;

    match &view.resolution {
        ConfigResolution::Loaded(redacted) => {
            writeln!(out, "resolved:")?;
            for line in redacted.json().lines() {
                writeln!(out, "  {}", safe_terminal_text(line))?;
            }
        }
        ConfigResolution::Rejected { reason } => {
            writeln!(out, "rejected: {}", safe_terminal_text(reason))?;
            writeln!(
                out,
                "{DIM}The paths above are reported so the refusal can be repaired.{RESET}"
            )?;
        }
    }

    if !view.diagnostics.is_empty() {
        writeln!(out, "permissions:")?;
        for finding in &view.diagnostics {
            writeln!(
                out,
                "  {} {} | expected {}{}",
                if finding.is_ok() { "ok" } else { "insecure" },
                safe_terminal_text(&finding.path.display().to_string()),
                safe_terminal_text(&finding.expected),
                match &finding.actual {
                    Some(actual) => format!(" | actual {}", safe_terminal_text(actual)),
                    None => String::new(),
                }
            )?;
            writeln!(
                out,
                "    {DIM}{}{RESET}",
                safe_terminal_text(&finding.message)
            )?;
        }
    }

    writeln!(
        out,
        "{DIM}Secrets appear as references only; no credential value is read to render this.{RESET}"
    )?;
    writeln!(out)
}

fn render_layer(
    out: &mut impl Write,
    label: &str,
    path: &std::path::Path,
    present: bool,
) -> io::Result<()> {
    writeln!(
        out,
        "{label:<8} {} | {}",
        safe_terminal_text(&path.display().to_string()),
        if present { "present" } else { "absent" }
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use orchester_laufzeit::harness::config::{ConfigLoader, PermissionDiagnostic};

    use super::*;

    fn diagnostic(path: &str) -> PermissionDiagnostic {
        PermissionDiagnostic {
            path: PathBuf::from(path),
            secure: false,
            expected: "owner-only access".into(),
            actual: Some("BUILTIN\\Users:(W)".into()),
            message: "configuration is writable by other users".into(),
        }
    }

    fn view(resolution: ConfigResolution) -> SelfAgentConfigView {
        SelfAgentConfigView {
            user_path: PathBuf::from("D:\\home\\.orchester\\orchester.jsonc"),
            user_present: true,
            project_path: PathBuf::from("D:\\repo\\.orchester\\project.jsonc"),
            project_present: false,
            resolution,
            diagnostics: Vec::new(),
        }
    }

    fn loaded(source: &str) -> ConfigResolution {
        let config = ConfigLoader::test().load_user(source).expect("config");
        ConfigResolution::Loaded(config.redacted())
    }

    fn render(view: &SelfAgentConfigView) -> String {
        let mut output = Vec::new();
        render_config(&mut output, view).expect("render config");
        String::from_utf8(output).expect("UTF-8")
    }

    #[test]
    fn a_loaded_configuration_reports_both_layers_and_the_redacted_body() {
        let rendered = render(&view(loaded(
            r#"{ "version": 1, "model": "gpt-5.6-sol", "model_provider": "OpenAI" }"#,
        )));

        // Both layers are named whether or not they exist, because "absent" is
        // the answer to "why is my setting being ignored" as often as "wrong".
        assert!(rendered.contains("D:\\home\\.orchester\\orchester.jsonc"));
        assert!(rendered.contains("D:\\repo\\.orchester\\project.jsonc"));
        assert!(rendered.contains("present"));
        assert!(rendered.contains("absent"));
        assert!(rendered.contains("gpt-5.6-sol"));
    }

    /// The reason this command exists. Every other command dies with one bare
    /// sentence; this one must still name the file that has to be repaired.
    #[test]
    fn a_rejected_configuration_still_names_the_file_and_the_reason() {
        let rendered = render(&view(ConfigResolution::Rejected {
            reason: "protected configuration file failed secure handle validation".into(),
        }));

        assert!(rendered.contains("D:\\home\\.orchester\\orchester.jsonc"));
        assert!(rendered.contains("failed secure handle validation"));
    }

    #[test]
    fn permission_findings_report_what_was_expected_and_what_was_found() {
        let mut projection = view(loaded("{}"));
        projection.diagnostics = vec![diagnostic("D:\\home\\.orchester")];

        let rendered = render(&projection);

        assert!(rendered.contains("D:\\home\\.orchester"));
        assert!(rendered.contains("owner-only access"));
        assert!(rendered.contains("BUILTIN\\Users:(W)"));
    }

    /// A project config is untrusted input and a rejection reason quotes it, so
    /// neither may smuggle a control sequence into the operator's terminal.
    #[test]
    fn configuration_text_cannot_forge_terminal_control_sequences() {
        let mut projection = view(ConfigResolution::Rejected {
            reason: "unknown field `model\u{1b}[31m`".into(),
        });
        projection.diagnostics = vec![PermissionDiagnostic {
            actual: Some("owner\u{1b}[31m".into()),
            ..diagnostic("D:\\home\\.orchester")
        }];

        let rendered = render(&projection);

        assert!(rendered.contains("model\\u{1b}[31m"));
        assert!(rendered.contains("owner\\u{1b}[31m"));
        assert!(!rendered.contains("\u{1b}[31m"));
    }
}
