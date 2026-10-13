//! Workspace-scoped, private and revision-checked composer context.
use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    health::no_store_headers,
    ServerContext,
};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::HeaderMap,
    Json,
};
use orchester_anwendung::OrchesterPaths;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

pub(crate) type Result<T> = std::result::Result<T, ApiErrorCode>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GoalStatus {
    Active,
    Paused,
    Completed,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Goal {
    pub text: String,
    pub status: GoalStatus,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ComposerContext {
    pub revision: i64,
    pub goal: Option<Goal>,
    pub plan_mode: bool,
}
impl ComposerContext {
    pub fn validate(&self) -> Result<()> {
        if self.revision < 0
            || self.revision == i64::MAX
            || self
                .goal
                .as_ref()
                .is_some_and(|g| !valid_text(&g.text, 4000))
        {
            return Err(ApiErrorCode::ValidationFailed);
        }
        Ok(())
    }
    pub fn prompt(&self, prompt: &str) -> String {
        let mut parts = Vec::new();
        if let Some(goal) = &self.goal {
            if goal.status == GoalStatus::Active {
                parts.push(format!("The user has set this ongoing goal for this workspace. Continue making progress toward it within the current request and report remaining work. Do not claim completion without evidence.\n<user-goal>\n{}\n</user-goal>", goal.text));
            }
        }
        if self.plan_mode {
            parts.push("Plan mode is enabled by the user. Inspect and explain as needed, then present a concrete plan. Do not propose or perform file changes, commands, installations, or external mutations. Wait for the user to disable plan mode before implementation.".into());
        }
        parts.push(prompt.to_owned());
        parts.join("\n\n")
    }
}
pub(crate) fn valid_text(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
}
pub(crate) struct ComposerStore {
    pub db: Connection,
    pub workspace: String,
}
impl ComposerStore {
    pub fn open(paths: &OrchesterPaths) -> Result<Self> {
        let db = orchester_laufzeit::harness::run_store::open_private_database(
            &paths.state_root().join("composer.db"),
        )
        .map_err(|_| ApiErrorCode::Unavailable)?;
        Self::with_connection(db, paths.workspace().to_string_lossy().into_owned())
    }
    fn with_connection(db: Connection, workspace: String) -> Result<Self> {
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| ApiErrorCode::Unavailable)?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS composer_context(workspace TEXT PRIMARY KEY, revision INTEGER NOT NULL, data TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS mcp_servers(id TEXT PRIMARY KEY, workspace TEXT NOT NULL, data TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS mcp_workspace ON mcp_servers(workspace);").map_err(|_|ApiErrorCode::Unavailable)?;
        Ok(Self { db, workspace })
    }
    pub fn context(&self) -> Result<ComposerContext> {
        let data: Option<String> = self
            .db
            .query_row(
                "SELECT data FROM composer_context WHERE workspace=?1",
                [&self.workspace],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ApiErrorCode::Unavailable)?;
        data.map(|s| serde_json::from_str(&s).map_err(|_| ApiErrorCode::Unavailable))
            .unwrap_or_else(|| Ok(ComposerContext::default()))
    }
    pub fn save_context(&mut self, mut input: ComposerContext) -> Result<ComposerContext> {
        input.validate()?;
        if let Some(g) = &mut input.goal {
            g.text = g.text.trim().to_owned();
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ApiErrorCode::Unavailable)?;
        let revision: i64 = tx
            .query_row(
                "SELECT revision FROM composer_context WHERE workspace=?1",
                [&self.workspace],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ApiErrorCode::Unavailable)?
            .unwrap_or(0);
        if input.revision != revision {
            return Err(ApiErrorCode::Conflict);
        }
        input.revision += 1;
        let data = serde_json::to_string(&input).map_err(|_| ApiErrorCode::Internal)?;
        tx.execute("INSERT INTO composer_context(workspace,revision,data) VALUES(?1,?2,?3) ON CONFLICT(workspace) DO UPDATE SET revision=excluded.revision,data=excluded.data", params![self.workspace,input.revision,data]).map_err(|_|ApiErrorCode::Unavailable)?;
        tx.commit().map_err(|_| ApiErrorCode::Unavailable)?;
        Ok(input)
    }
}
pub(crate) async fn storage<T: Send + 'static>(
    context: &ServerContext,
    op: impl FnOnce(&mut ComposerStore) -> Result<T> + Send + 'static,
) -> Result<T> {
    let paths = context.paths().cloned().ok_or(ApiErrorCode::Unavailable)?;
    tokio::task::spawn_blocking(move || op(&mut ComposerStore::open(&paths)?))
        .await
        .map_err(|_| ApiErrorCode::Internal)?
}
pub(crate) async fn get(
    State(context): State<ServerContext>,
    headers: HeaderMap,
) -> std::result::Result<(HeaderMap, Json<ComposerContext>), ApiErrorResponse> {
    let value = storage(&context, |s| s.context())
        .await
        .map_err(|e| api_error_response(e, request_id_from_headers(&headers)))?;
    Ok((no_store_headers(), Json(value)))
}
pub(crate) async fn put(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    input: std::result::Result<Json<ComposerContext>, JsonRejection>,
) -> std::result::Result<(HeaderMap, Json<ComposerContext>), ApiErrorResponse> {
    let Json(input) = input.map_err(|_| {
        api_error_response(ApiErrorCode::BadRequest, request_id_from_headers(&headers))
    })?;
    let value = storage(&context, move |s| s.save_context(input))
        .await
        .map_err(|e| api_error_response(e, request_id_from_headers(&headers)))?;
    Ok((no_store_headers(), Json(value)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn goals_persist_with_revision_and_paused_goals_do_not_affect_prompts() {
        let mut store =
            ComposerStore::with_connection(Connection::open_in_memory().unwrap(), "a".into())
                .unwrap();
        let context = ComposerContext {
            goal: Some(Goal {
                text: " Finish report ".into(),
                status: GoalStatus::Active,
            }),
            plan_mode: true,
            ..Default::default()
        };
        let mut saved = store.save_context(context.clone()).unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(store.context().unwrap(), saved);
        assert!(matches!(
            store.save_context(context),
            Err(ApiErrorCode::Conflict)
        ));
        assert!(saved.prompt("Next").contains("Finish report"));
        saved.goal.as_mut().unwrap().status = GoalStatus::Paused;
        let saved = store.save_context(saved).unwrap();
        assert!(!saved.prompt("Next").contains("Finish report"));
        assert!(saved.prompt("Next").contains("Plan mode"));
        store.workspace = "b".into();
        assert_eq!(store.context().unwrap(), ComposerContext::default());
    }
}
