use orchester_protokoll::{ActionId, CallId, RunId, StepId, TurnId};

use super::super::run_store::{ResumeNext, RunSnapshot, RunStore, SqliteRunStore, StoreError};
use super::super::transcript::TranscriptRecord;

/// Fresh identifiers and owner scope for one model step resumed from disk.
#[derive(Clone, PartialEq, Eq)]
pub struct CoordinatorContinuationInput {
    pub run_id: RunId,
    pub owner_actor_id: String,
    pub step_id: StepId,
    pub model_call_id: CallId,
    pub action_id: ActionId,
}

impl std::fmt::Debug for CoordinatorContinuationInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoordinatorContinuationInput")
            .field("run_id", &"<redacted>")
            .field("owner_actor_id", &"<redacted>")
            .field("step_id", &"<redacted>")
            .field("model_call_id", &"<redacted>")
            .field("action_id", &"<redacted>")
            .finish()
    }
}

/// Store-validated history and counters required to reconstruct a model step.
pub struct CoordinatorContinuationState {
    pub run: RunSnapshot,
    pub turn_id: TurnId,
    pub transcript: Vec<TranscriptRecord>,
}

impl std::fmt::Debug for CoordinatorContinuationState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CoordinatorContinuationState")
            .field("run_id", &"<redacted>")
            .field("turn_id", &"<redacted>")
            .field("steps_used", &self.run.steps_used)
            .field("transcript_records", &self.transcript.len())
            .finish()
    }
}

pub(super) fn load_sqlite_continuation(
    store: &SqliteRunStore,
    run_id: &RunId,
    owner_actor_id: &str,
) -> Result<CoordinatorContinuationState, StoreError> {
    let run = <SqliteRunStore as RunStore>::load_run_owned(store, run_id, owner_actor_id)?;
    let resume = store
        .resume_point_owned(run_id, owner_actor_id, &run.project_id)?
        .ok_or_else(|| StoreError::Invariant("run cannot continue".into()))?;
    if !matches!(resume.next, ResumeNext::StartNextStep) {
        return Err(StoreError::Invariant(
            "run is not ready for a model continuation".into(),
        ));
    }
    let turn_id = resume
        .turn_id
        .ok_or_else(|| StoreError::Invariant("continuation turn is missing".into()))?;
    let transcript = store
        .transcript_owned(run_id, owner_actor_id)?
        .into_iter()
        .map(|stored| stored.record)
        .collect();
    Ok(CoordinatorContinuationState {
        run,
        turn_id,
        transcript,
    })
}
