use orchester_laufzeit::harness::governance::PolicyEngine;
use orchester_laufzeit::harness::run_store::{
    action_hash, ActionRecord, EventAppend, NewRun, RunStore, SqliteRunStore, Transition,
};
use orchester_protokoll::{
    ActionId, AgentAction, CallId, HarnessEventKind, Observation, ObservationId, PolicyDecision,
    RunId, StepId, TurnId,
};

pub(crate) struct AllowedRun {
    pub(crate) run_id: RunId,
    pub(crate) turn_id: TurnId,
    pub(crate) step_id: StepId,
    pub(crate) action_id: ActionId,
    pub(crate) owner: String,
    pub(crate) provider_call_id: CallId,
}

impl AllowedRun {
    pub(crate) fn tool_started_input(&self) -> EventAppend {
        EventAppend {
            turn_id: Some(self.turn_id.clone()),
            step_id: Some(self.step_id.clone()),
            call_id: Some(self.provider_call_id.clone()),
            occurred_at: "2026-07-12T00:00:10Z".into(),
            kind: HarnessEventKind::ToolStarted {
                action_id: self.action_id.clone(),
            },
        }
    }

    pub(crate) fn tool_completed_input(&self, call_id: &CallId) -> EventAppend {
        EventAppend {
            turn_id: Some(self.turn_id.clone()),
            step_id: Some(self.step_id.clone()),
            call_id: Some(call_id.clone()),
            occurred_at: "2026-07-12T00:00:11Z".into(),
            kind: HarnessEventKind::ToolCompleted {
                observation: Observation {
                    observation_id: ObservationId::from(format!(
                        "observation-{}-{}",
                        self.run_id.0, call_id.0
                    )),
                    call_id: call_id.clone(),
                    kind: "read_file".into(),
                    summary: "ok".into(),
                    data: serde_json::json!({"bytes": 0}),
                },
            },
        }
    }
}

pub(crate) fn create_allowed_run(store: &SqliteRunStore, label: &str) -> AllowedRun {
    let run_id = RunId::from(format!("run-{label}"));
    let turn_id = TurnId::from(format!("turn-{label}"));
    let step_id = StepId::from(format!("step-{label}"));
    let action_id = ActionId::from(format!("action-{label}"));
    let model_call_id = CallId::from(format!("model-call-{label}"));
    let provider_call_id = CallId::from(format!("provider-call-{label}"));
    let owner = format!("owner-{label}");

    store
        .create_run(NewRun {
            run_id: run_id.clone(),
            project_id: format!("project-{label}"),
            owner_actor_id: owner.clone(),
            canonical_root: format!("/workspace/{label}"),
            workspace_identity: format!("workspace-{label}"),
            policy_snapshot_hash: format!("policy-{label}"),
            config_snapshot_hash: format!("config-{label}"),
            max_steps: 4,
            occurred_at: "2026-07-12T00:00:00Z".into(),
        })
        .unwrap();
    store
        .append_transition(
            &run_id,
            &owner,
            Transition::StartStep {
                turn_id: turn_id.clone(),
                step_id: step_id.clone(),
                occurred_at: "2026-07-12T00:00:01Z".into(),
            },
        )
        .unwrap();
    store
        .append_event(
            &owner,
            &run_id,
            EventAppend {
                turn_id: Some(turn_id.clone()),
                step_id: Some(step_id.clone()),
                call_id: Some(model_call_id.clone()),
                occurred_at: "2026-07-12T00:00:02Z".into(),
                kind: HarnessEventKind::ModelStarted,
            },
        )
        .unwrap();
    store
        .append_event(
            &owner,
            &run_id,
            EventAppend {
                turn_id: Some(turn_id.clone()),
                step_id: Some(step_id.clone()),
                call_id: Some(model_call_id.clone()),
                occurred_at: "2026-07-12T00:00:03Z".into(),
                kind: HarnessEventKind::ModelCompleted {
                    assistant_text: String::new(),
                },
            },
        )
        .unwrap();
    let action = AgentAction::ReadFile {
        path: format!("src/{label}.rs"),
        start_line: None,
        end_line: None,
    };
    store
        .record_action(
            &owner,
            ActionRecord {
                action_id: action_id.clone(),
                run_id: run_id.clone(),
                step_id: step_id.clone(),
                call_id: provider_call_id.clone(),
                origin_model_call_id: model_call_id,
                action_hash: action_hash(&action).unwrap(),
                effect_class: PolicyEngine::new().evaluate(&action).unwrap().effect,
                action,
                occurred_at: "2026-07-12T00:00:04Z".into(),
            },
        )
        .unwrap();
    store
        .append_event(
            &owner,
            &run_id,
            EventAppend {
                turn_id: Some(turn_id.clone()),
                step_id: Some(step_id.clone()),
                call_id: None,
                occurred_at: "2026-07-12T00:00:05Z".into(),
                kind: HarnessEventKind::PolicyDecided {
                    action_id: action_id.clone(),
                    decision: PolicyDecision::Allow,
                    rule_id: "workspace.read".into(),
                },
            },
        )
        .unwrap();

    AllowedRun {
        run_id,
        turn_id,
        step_id,
        action_id,
        owner,
        provider_call_id,
    }
}
