use std::io::{self, Write};

use orchester_laufzeit::harness::service::{
    SelfAgentResumeAvailability, SelfAgentResumeCatalog, SelfAgentResumeEntry,
    SelfAgentResumeStage, SelfAgentResumeStep,
};

use super::render::safe_terminal_text;

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn render_resume(out: &mut impl Write, catalog: &SelfAgentResumeCatalog) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{BOLD}Resumable self-agent runs{RESET}")?;
    if !catalog.database_present || catalog.entries.is_empty() {
        writeln!(out, "no resumable runs")?;
    } else {
        for entry in &catalog.entries {
            render_entry(out, entry)?;
        }
        if catalog.truncated {
            writeln!(out, "{DIM}showing the newest bounded entries only{RESET}")?;
        }
    }
    writeln!(out, "{DIM}Read-only catalog; no run was continued.{RESET}")?;
    writeln!(out)
}

fn render_entry(out: &mut impl Write, entry: &SelfAgentResumeEntry) -> io::Result<()> {
    let marker = if entry.latest { "*" } else { " " };
    writeln!(
        out,
        "{marker} {} | {} | {}",
        safe_terminal_text(&entry.handle),
        availability_name(entry.availability),
        step_name(entry.step)
    )
}

fn availability_name(value: SelfAgentResumeAvailability) -> &'static str {
    match value {
        SelfAgentResumeAvailability::Ready => "ready",
        SelfAgentResumeAvailability::ApprovalRequired => "approval required",
        SelfAgentResumeAvailability::ReconciliationRequired => "reconciliation required",
    }
}

fn step_name(value: SelfAgentResumeStep) -> String {
    match value {
        SelfAgentResumeStep::StartStep => "start step".into(),
        SelfAgentResumeStep::StartModel => "start model".into(),
        SelfAgentResumeStep::ProcessModelOutput => "process model output".into(),
        SelfAgentResumeStep::EvaluatePolicy => "evaluate policy".into(),
        SelfAgentResumeStep::PrepareExecution => "prepare execution".into(),
        SelfAgentResumeStep::StartNextStep => "start next step".into(),
        SelfAgentResumeStep::ContinueValidation => "continue validation".into(),
        SelfAgentResumeStep::CreateApprovalRequest => "create approval request".into(),
        SelfAgentResumeStep::AwaitApproval => "await approval".into(),
        SelfAgentResumeStep::RecoverApprovalCapability => "recover approval capability".into(),
        SelfAgentResumeStep::ReconcileModelCall => "reconcile model call".into(),
        SelfAgentResumeStep::ReconcileToolOutcome => "reconcile tool outcome".into(),
        SelfAgentResumeStep::ManualReconciliation(stage) => {
            format!("manual reconciliation ({})", stage_name(stage))
        }
    }
}

fn stage_name(value: SelfAgentResumeStage) -> &'static str {
    match value {
        SelfAgentResumeStage::MissingStep => "missing step",
        SelfAgentResumeStage::ModelCall => "model call",
        SelfAgentResumeStage::ToolOutcome => "tool outcome",
        SelfAgentResumeStage::UnboundApproval => "unbound approval",
        SelfAgentResumeStage::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_is_bounded_and_does_not_echo_internal_identifiers() {
        let catalog = SelfAgentResumeCatalog {
            database_present: true,
            truncated: true,
            entries: vec![SelfAgentResumeEntry {
                handle: "r-opaque\x1b[31m".into(),
                availability: SelfAgentResumeAvailability::ReconciliationRequired,
                step: SelfAgentResumeStep::ManualReconciliation(SelfAgentResumeStage::ModelCall),
                latest: true,
            }],
        };
        let mut output = Vec::new();

        render_resume(&mut output, &catalog).expect("render resume catalog");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("Resumable self-agent runs"));
        assert!(rendered.contains("r-opaque\\u{1b}[31m"));
        assert!(rendered.contains("reconciliation required | manual reconciliation (model call)"));
        assert!(rendered.contains("showing the newest bounded entries only"));
        assert!(!rendered.contains("r-opaque\x1b[31m"));
        assert!(rendered.contains("no run was continued"));
    }

    #[test]
    fn rendering_empty_catalog_is_explicit() {
        let catalog = SelfAgentResumeCatalog {
            database_present: false,
            truncated: false,
            entries: Vec::new(),
        };
        let mut output = Vec::new();

        render_resume(&mut output, &catalog).expect("render empty catalog");
        let rendered = String::from_utf8(output).expect("UTF-8");

        assert!(rendered.contains("no resumable runs"));
    }
}
