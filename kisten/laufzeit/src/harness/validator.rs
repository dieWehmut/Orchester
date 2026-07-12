//! Structured validator specifications and injected process-result handling.
//!
//! This module deliberately does not spawn processes.  A future process
//! runtime supplies [`ProcessResult`] after applying policy, sandbox, timeout,
//! and cancellation controls; the validator state machine then remains fully
//! deterministic and offline-testable.

use std::path::Path;

use orchester_protokoll::FeedbackReport;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::feedback::{FeedbackClass, FeedbackEngine, FeedbackInput, FeedbackTruncation};
use super::mutation::{MutationObservation, MutationTracker, SnapshotResult};
use super::run_store::RunSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorSpec {
    pub id: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidatorSpecError {
    #[error("validator id cannot be empty")]
    EmptyId,
    #[error("validator program cannot be empty")]
    EmptyProgram,
    #[error("validator program or arguments contain a control character")]
    ControlCharacter,
    #[error("validator cannot invoke a shell interpreter")]
    ShellInterpreter,
    #[error("validator arguments cannot contain shell wrapper flags")]
    ShellWrapper,
}

impl ValidatorSpec {
    pub fn new(
        id: impl Into<String>,
        program: impl Into<String>,
        args: Vec<String>,
        required: bool,
    ) -> Result<Self, ValidatorSpecError> {
        let spec = Self {
            id: id.into(),
            program: program.into(),
            args,
            required,
            timeout_ms: None,
        };
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), ValidatorSpecError> {
        if self.id.trim().is_empty() {
            return Err(ValidatorSpecError::EmptyId);
        }
        if self.program.trim().is_empty() {
            return Err(ValidatorSpecError::EmptyProgram);
        }
        if self
            .program
            .chars()
            .chain(self.id.chars())
            .chain(self.args.iter().flat_map(|arg| arg.chars()))
            .any(char::is_control)
        {
            return Err(ValidatorSpecError::ControlCharacter);
        }
        // A bare program is one executable token. Paths may contain spaces,
        // but a whitespace-containing value with no path separator is a
        // composite command pasted into the structured field.
        if self.program.chars().any(char::is_whitespace) && !self.program.contains(['/', '\\']) {
            return Err(ValidatorSpecError::ShellWrapper);
        }
        let basename = self
            .program
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&self.program)
            .to_ascii_lowercase();
        let basename = basename.strip_suffix(".exe").unwrap_or(&basename);
        if matches!(
            basename,
            "sh" | "bash"
                | "zsh"
                | "fish"
                | "dash"
                | "cmd"
                | "powershell"
                | "pwsh"
                | "wscript"
                | "cscript"
        ) {
            return Err(ValidatorSpecError::ShellInterpreter);
        }
        if self.args.iter().any(|arg| {
            matches!(
                arg.to_ascii_lowercase().as_str(),
                "-c" | "/c" | "-command" | "/command" | "-encodedcommand"
            )
        }) {
            return Err(ValidatorSpecError::ShellWrapper);
        }
        Ok(())
    }

    pub fn executable_path(&self) -> &Path {
        Path::new(&self.program)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessTermination {
    Exited(i32),
    Cancelled,
    TimedOut,
    SpawnFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessResult {
    pub termination: ProcessTermination,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessResult {
    pub fn exited(code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            termination: ProcessTermination::Exited(code),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    pub fn cancelled() -> Self {
        Self {
            termination: ProcessTermination::Cancelled,
            stdout: String::new(),
            stderr: String::new(),
        }
    }

    pub fn timed_out() -> Self {
        Self {
            termination: ProcessTermination::TimedOut,
            stdout: String::new(),
            stderr: String::new(),
        }
    }

    pub fn spawn_failed(stderr: impl Into<String>) -> Self {
        Self {
            termination: ProcessTermination::SpawnFailed,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidatorClassification {
    Passed,
    ExitFailure,
    Cancelled,
    TimedOut,
    SpawnFailed,
    MutatedSources,
    OutputTruncated,
    SnapshotLimitExceeded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatorEvaluation {
    pub classification: ValidatorClassification,
    pub report: FeedbackReport,
    pub truncation: FeedbackTruncation,
    pub mutation: MutationObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorState {
    pub id: String,
    pub required: bool,
    pub last_passed_generation: Option<u64>,
    pub last_feedback_fingerprint: Option<String>,
}

impl ValidatorState {
    pub fn new(id: impl Into<String>, required: bool) -> Self {
        Self {
            id: id.into(),
            required,
            last_passed_generation: None,
            last_feedback_fingerprint: None,
        }
    }

    pub fn passed(id: impl Into<String>, required: bool, generation: u64) -> Self {
        Self {
            id: id.into(),
            required,
            last_passed_generation: Some(generation),
            last_feedback_fingerprint: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishBlocked {
    ValidationRequired,
}

pub fn can_finish(run: &RunSnapshot, validators: &[ValidatorState]) -> Result<(), FinishBlocked> {
    validators
        .iter()
        .filter(|validator| validator.required)
        .all(|validator| validator.last_passed_generation == Some(run.mutation_generation))
        .then_some(())
        .ok_or(FinishBlocked::ValidationRequired)
}

#[derive(Debug, Clone, Default)]
pub struct ValidatorEngine {
    feedback: FeedbackEngine,
}

impl ValidatorEngine {
    pub fn new(feedback: FeedbackEngine) -> Self {
        Self { feedback }
    }

    pub fn feedback_engine(&self) -> &FeedbackEngine {
        &self.feedback
    }

    pub fn evaluate(
        &self,
        spec: &ValidatorSpec,
        state: &mut ValidatorState,
        tracker: &mut MutationTracker,
        before: &SnapshotResult,
        after: &SnapshotResult,
        process: ProcessResult,
    ) -> ValidatorEvaluation {
        // A spec is normally validated at configuration load. Keep this
        // boundary defensive without introducing a process side effect.
        let mutation = tracker.observe(before, after);
        let invalid_spec =
            spec.validate().is_err() || state.id != spec.id || state.required != spec.required;
        let (mut classification, class, exit_code, retryable) = if mutation.uncertain {
            (
                ValidatorClassification::SnapshotLimitExceeded,
                FeedbackClass::SnapshotLimitExceeded,
                None,
                false,
            )
        } else if mutation.changed {
            (
                ValidatorClassification::MutatedSources,
                FeedbackClass::ValidatorMutatedSources,
                termination_exit_code(&process.termination),
                false,
            )
        } else if invalid_spec {
            (
                ValidatorClassification::SpawnFailed,
                FeedbackClass::ProcessSpawnFailed,
                None,
                false,
            )
        } else {
            match process.termination {
                ProcessTermination::Exited(0) => (
                    ValidatorClassification::Passed,
                    FeedbackClass::ValidatorPassed,
                    Some(0),
                    false,
                ),
                ProcessTermination::Exited(code) => (
                    ValidatorClassification::ExitFailure,
                    FeedbackClass::ValidatorFailed,
                    Some(code),
                    true,
                ),
                ProcessTermination::Cancelled => (
                    ValidatorClassification::Cancelled,
                    FeedbackClass::ProcessCancelled,
                    None,
                    false,
                ),
                ProcessTermination::TimedOut => (
                    ValidatorClassification::TimedOut,
                    FeedbackClass::ProcessTimedOut,
                    None,
                    true,
                ),
                ProcessTermination::SpawnFailed => (
                    ValidatorClassification::SpawnFailed,
                    FeedbackClass::ProcessSpawnFailed,
                    None,
                    false,
                ),
            }
        };

        let stdout = process.stdout;
        let stderr = process.stderr;
        let mut built = self.feedback.build(FeedbackInput {
            source: "validator".into(),
            validator_id: Some(spec.id.clone()),
            exit_code,
            class,
            summary: summary_for(class, exit_code),
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            retryable,
        });
        if classification == ValidatorClassification::Passed
            && spec.required
            && built.truncated.any()
        {
            let truncation = built.truncated;
            classification = ValidatorClassification::OutputTruncated;
            built = self.feedback.build(FeedbackInput {
                source: "validator".into(),
                validator_id: Some(spec.id.clone()),
                exit_code: Some(0),
                class: FeedbackClass::ValidatorOutputTruncated,
                summary: summary_for(FeedbackClass::ValidatorOutputTruncated, Some(0)),
                stdout,
                stderr,
                retryable: false,
            });
            // The second build has a different class but the same bounded
            // source streams; retain the original truncation evidence.
            built.truncated = truncation;
        }

        state.last_feedback_fingerprint = Some(built.report.fingerprint.clone());
        if classification == ValidatorClassification::Passed {
            state.last_passed_generation = Some(mutation.generation);
        } else {
            state.last_passed_generation = None;
        }
        ValidatorEvaluation {
            classification,
            report: built.report,
            truncation: built.truncated,
            mutation,
        }
    }
}

fn termination_exit_code(termination: &ProcessTermination) -> Option<i32> {
    match termination {
        ProcessTermination::Exited(code) => Some(*code),
        _ => None,
    }
}

fn summary_for(class: FeedbackClass, exit_code: Option<i32>) -> String {
    match class {
        FeedbackClass::ValidatorPassed => "validator passed".into(),
        FeedbackClass::ValidatorOutputTruncated => "validator output was truncated".into(),
        FeedbackClass::SnapshotLimitExceeded => "source snapshot exceeded configured limits".into(),
        FeedbackClass::ValidatorMutatedSources => "validator mutated source files".into(),
        FeedbackClass::ValidatorFailed => format!(
            "validator exited unsuccessfully ({})",
            exit_code.unwrap_or(-1)
        ),
        FeedbackClass::ProcessCancelled => "validator was cancelled".into(),
        FeedbackClass::ProcessTimedOut => "validator timed out".into(),
        FeedbackClass::ProcessSpawnFailed => "validator could not be started".into(),
        _ => class.as_str().replace('_', " "),
    }
}
