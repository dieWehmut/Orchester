use axum::{extract::State, http::HeaderMap, Json};
use orchester_anwendung::{WorkspaceReview, WorkspaceReviewError, WorkspaceReviewKind};
use serde::Serialize;

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    bootstrap::ServerContext,
    health::no_store_headers,
};

pub const WORKSPACE_REVIEW_SCHEMA_VERSION: u8 = 1;

/// A path's change in the decoration vocabulary the diff list renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceReviewKindDto {
    Added,
    Modified,
    Deleted,
    Untracked,
}

/// One changed path, relative to the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceReviewChangeDto {
    pub path: String,
    pub kind: WorkspaceReviewKindDto,
    pub staged: bool,
    pub unstaged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceReviewDto {
    pub schema_version: u8,
    /// The branch HEAD points at, or null when HEAD is detached.
    pub branch: Option<String>,
    pub changes: Vec<WorkspaceReviewChangeDto>,
    pub branch_changes: Vec<String>,
}

pub fn workspace_review_response(review: &WorkspaceReview) -> WorkspaceReviewDto {
    WorkspaceReviewDto {
        schema_version: WORKSPACE_REVIEW_SCHEMA_VERSION,
        branch: review.branch.clone(),
        changes: review
            .changes
            .iter()
            .map(|change| WorkspaceReviewChangeDto {
                path: change.path.clone(),
                kind: match change.kind {
                    WorkspaceReviewKind::Added => WorkspaceReviewKindDto::Added,
                    WorkspaceReviewKind::Modified => WorkspaceReviewKindDto::Modified,
                    WorkspaceReviewKind::Deleted => WorkspaceReviewKindDto::Deleted,
                    WorkspaceReviewKind::Untracked => WorkspaceReviewKindDto::Untracked,
                },
                staged: change.staged,
                unstaged: change.unstaged,
            })
            .collect(),
        branch_changes: review.branch_changes.clone(),
    }
}

/// Read the working copy's review state for the selected workspace.
///
/// A plain directory answers with an empty review rather than an error. That is
/// deliberate: Orchester runs against directories that are not repositories, and
/// an empty Review tab is the designed state for one (P6). A workspace that is a
/// repository but cannot be read is a real failure, and is reported as one.
pub(crate) async fn workspace_review_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<WorkspaceReviewDto>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let workspace = context
        .paths()
        .map(|paths| paths.workspace().to_path_buf())
        .ok_or_else(|| api_error_response(ApiErrorCode::Unavailable, request_id))?;

    // `git` is a blocking child process; the loopback server must not read a
    // slow disk on a runtime worker.
    let review = tokio::task::spawn_blocking(move || WorkspaceReview::read(&workspace))
        .await
        .map_err(|_| api_error_response(ApiErrorCode::Internal, request_id))?;

    let review = match review {
        Ok(review) => review,
        Err(WorkspaceReviewError::NotARepository) => WorkspaceReview {
            branch: None,
            changes: Vec::new(),
            branch_changes: Vec::new(),
        },
        Err(WorkspaceReviewError::Unreadable) => {
            return Err(api_error_response(ApiErrorCode::Unavailable, request_id))
        }
    };

    Ok((no_store_headers(), Json(workspace_review_response(&review))))
}
