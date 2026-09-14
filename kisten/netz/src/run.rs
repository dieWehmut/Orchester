//! HTTP boundary for local runs.
//!
//! The first route slice deliberately reports an explicit availability error
//! until a context has a runtime manager.  Keeping the handlers in place makes
//! the wire contract observable and gives later runtime work a stable seam.

use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::HeaderMap,
    Json,
};
use orchester_protokoll::RunId;

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    health::no_store_headers,
    run_contract::{
        RunReplayRequestDto, RunReplayResponseDto, RunSnapshotDto, RunSummaryDto, StartRunRequest,
    },
    run_registry::{RunHandle, RunRegistryError},
    ServerContext,
};

pub(crate) async fn start_run_handler(
    State(_context): State<ServerContext>,
    headers: HeaderMap,
    request: Result<Json<StartRunRequest>, JsonRejection>,
) -> Result<(HeaderMap, Json<serde_json::Value>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let Json(request) =
        request.map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id))?;
    request
        .validate()
        .map_err(|_| api_error_response(ApiErrorCode::ValidationFailed, request_id))?;
    Err(api_error_response(ApiErrorCode::Unavailable, request_id))
}

pub(crate) async fn snapshot_run_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<(HeaderMap, Json<RunSnapshotDto>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let run = registered_run(&context, run_id, request_id)?;
    let snapshot = run
        .snapshot()
        .await
        .map_err(|error| run_error_response(error, request_id))?;
    Ok((no_store_headers(), Json(snapshot)))
}

pub(crate) async fn replay_run_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    request: Result<Json<RunReplayRequestDto>, JsonRejection>,
) -> Result<(HeaderMap, Json<RunReplayResponseDto>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let Json(request) =
        request.map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id))?;
    let limit = request
        .bounded_limit()
        .map_err(|_| api_error_response(ApiErrorCode::ValidationFailed, request_id))?;
    let run = registered_run(&context, run_id, request_id)?;
    let replay = run
        .replay(request.after_sequence, Some(limit))
        .await
        .map_err(|error| run_error_response(error, request_id))?;
    Ok((no_store_headers(), Json(replay)))
}

pub(crate) async fn cancel_run_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<(HeaderMap, Json<RunSummaryDto>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let run = registered_run(&context, run_id, request_id)?;
    let summary = run
        .cancel()
        .await
        .map_err(|error| run_error_response(error, request_id))?;
    Ok((no_store_headers(), Json(summary)))
}

fn registered_run(
    context: &ServerContext,
    run_id: String,
    request_id: Option<&str>,
) -> Result<RunHandle, ApiErrorResponse> {
    context
        .runs()
        .get(&RunId::from(run_id))
        .map_err(|error| run_error_response(error, request_id))
}

fn run_error_response(error: RunRegistryError, request_id: Option<&str>) -> ApiErrorResponse {
    let code = match error {
        RunRegistryError::NotFound => ApiErrorCode::NotFound,
        RunRegistryError::RetentionExceeded { .. } => ApiErrorCode::ResyncRequired,
        RunRegistryError::InvalidLimit => ApiErrorCode::ValidationFailed,
        RunRegistryError::Terminal => ApiErrorCode::Conflict,
        RunRegistryError::Entropy
        | RunRegistryError::InvalidEvent(_)
        | RunRegistryError::LockPoisoned => ApiErrorCode::Internal,
    };
    api_error_response(code, request_id)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request, StatusCode},
        response::Response,
    };
    use orchester_protokoll::UiEventKind;
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;
    use crate::{app_router, ServerControl};

    #[tokio::test]
    async fn snapshot_returns_the_registered_run_state() {
        let context = ServerContext::new(None, ServerControl::new());
        let run = context.runs().create().expect("create run");
        run.append(UiEventKind::RunStarted {
            title: Some("Inspect workspace".into()),
        })
        .await
        .expect("append run start");

        let response = app_router(context)
            .oneshot(
                Request::get(format!("/api/v1/runs/{}", run.id().0))
                    .body(Body::empty())
                    .expect("snapshot request"),
            )
            .await
            .expect("snapshot response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&header::HeaderValue::from_static("no-store"))
        );
        let snapshot = json_body(response).await;
        assert_eq!(snapshot["run_id"], run.id().0);
        assert_eq!(snapshot["state"], "running");
        assert_eq!(snapshot["oldest_sequence"], 1);
        assert_eq!(snapshot["latest_sequence"], 1);
        assert_eq!(snapshot["next_sequence"], 2);
        assert_eq!(snapshot["events"][0]["kind"]["type"], "run_started");
    }

    #[tokio::test]
    async fn replay_returns_a_bounded_page_after_the_requested_sequence() {
        let context = ServerContext::new(None, ServerControl::new());
        let run = context.runs().create().expect("create run");
        for text in ["one", "two", "three"] {
            run.append(UiEventKind::Message { text: text.into() })
                .await
                .expect("append message");
        }

        let response = app_router(context)
            .oneshot(
                Request::post(format!("/api/v1/runs/{}/replay", run.id().0))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"after_sequence":1,"limit":1}"#))
                    .expect("replay request"),
            )
            .await
            .expect("replay response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&header::HeaderValue::from_static("no-store"))
        );
        let replay = json_body(response).await;
        assert_eq!(replay["run_id"], run.id().0);
        assert_eq!(replay["first_sequence"], 2);
        assert_eq!(replay["last_sequence"], 2);
        assert_eq!(replay["events"][0]["kind"]["text"], "two");
        assert_eq!(replay["has_more"], true);
    }

    #[tokio::test]
    async fn replay_requires_resync_when_the_requested_sequence_was_evicted() {
        let context = ServerContext::new(None, ServerControl::new());
        let run = context.runs().create().expect("create run");
        for index in 0..257 {
            run.append(UiEventKind::Message {
                text: format!("event {index}"),
            })
            .await
            .expect("append retained event");
        }

        let response = app_router(context)
            .oneshot(
                Request::post(format!("/api/v1/runs/{}/replay", run.id().0))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"after_sequence":0}"#))
                    .expect("replay request"),
            )
            .await
            .expect("replay response");

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let error = json_body(response).await;
        assert_eq!(error["code"], "resync_required");
    }

    #[tokio::test]
    async fn cancel_is_idempotent_and_emits_one_terminal_event() {
        let context = ServerContext::new(None, ServerControl::new());
        let run = context.runs().create().expect("create run");
        run.append(UiEventKind::RunStarted { title: None })
            .await
            .expect("append run start");
        let path = format!("/api/v1/runs/{}/cancel", run.id().0);

        for _ in 0..2 {
            let response = app_router(context.clone())
                .oneshot(
                    Request::post(&path)
                        .body(Body::empty())
                        .expect("cancel request"),
                )
                .await
                .expect("cancel response");

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL),
                Some(&header::HeaderValue::from_static("no-store"))
            );
            let summary = json_body(response).await;
            assert_eq!(summary["run_id"], run.id().0);
            assert_eq!(summary["stopped"], true);
        }

        let snapshot = run.snapshot().await.expect("cancelled snapshot");
        assert_eq!(snapshot.state, crate::run_contract::RunStateDto::Cancelled);
        assert_eq!(snapshot.events.len(), 2);
        assert!(run.cancellation_token().is_cancelled());
    }

    async fn json_body(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        serde_json::from_slice::<Value>(&body).expect("response JSON")
    }
}
