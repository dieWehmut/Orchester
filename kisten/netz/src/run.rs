//! HTTP boundary for local runs.
//!
//! The first route slice deliberately reports an explicit availability error
//! until a context has a runtime manager.  Keeping the handlers in place makes
//! the wire contract observable and gives later runtime work a stable seam.

use std::sync::Arc;

use axum::{
    extract::{
        rejection::JsonRejection,
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use orchester_anwendung::SelfAgentHost;
use orchester_laufzeit::harness::service::RunEventSink;
use orchester_protokoll::{RunId, StopReason, UiEventKind};
use serde::Deserialize;

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    health::no_store_headers,
    run_bridge::{drain_run_events, RegistryRunSink},
    run_contract::{
        RunReplayRequestDto, RunReplayResponseDto, RunSnapshotDto, RunStreamFrameDto,
        RunSummaryDto, StartRunRequest, StartRunResponse, RUN_IDEMPOTENCY_KEY_MAX_BYTES,
    },
    run_registry::{RunCreation, RunHandle, RunRegistryError},
    ServerContext,
};

pub(crate) async fn start_run_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    request: Result<Json<StartRunRequest>, JsonRejection>,
) -> Result<(HeaderMap, Json<StartRunResponse>), ApiErrorResponse> {
    let request_id = request_id_from_headers(&headers);
    let Json(request) =
        request.map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id))?;
    request
        .validate()
        .map_err(|_| api_error_response(ApiErrorCode::ValidationFailed, request_id))?;
    let idempotency_key = parse_idempotency_key(&headers)
        .map_err(|_| api_error_response(ApiErrorCode::ValidationFailed, request_id))?;

    // Without paths there is no configuration to run: the same availability
    // answer this route gave before a runtime existed.
    let paths = context
        .paths()
        .cloned()
        .ok_or_else(|| api_error_response(ApiErrorCode::Unavailable, request_id))?;

    // Snapshot the selected model before publishing a run. After publication
    // there must be no await before the detached task owns execution, otherwise
    // dropping an HTTP request could leave a permanently unstarted keyed run.
    let selection = context.model_selection().read().await;
    let composer = crate::composer::storage(&context, |s| s.context())
        .await
        .map_err(|code| api_error_response(code, request_id))?;
    let history = context
        .session_history()
        .ok_or_else(|| api_error_response(ApiErrorCode::Unavailable, request_id))?;
    let prior_context = match request.conversation_id.as_deref() {
        Some(id) => history.conversation_context(id).map_err(|error| {
            let code = if matches!(error, orchester_anwendung::SessionHistoryError::NotFound) {
                ApiErrorCode::NotFound
            } else {
                ApiErrorCode::Unavailable
            };
            api_error_response(code, request_id)
        })?,
        None => Vec::new(),
    };
    let mut conversation_id = String::new();
    let creation = context
        .runs()
        .create_or_reuse_initialized(
            idempotency_key.as_deref(),
            request.fingerprint(),
            UiEventKind::UserMessage {
                text: request.prompt.clone(),
            },
            |run| {
                conversation_id = history
                    .begin_conversation_with_event(
                        &run.initial_event()?,
                        request.conversation_id.as_deref(),
                        prior_context.len(),
                    )
                    .map_err(|_| RunRegistryError::HistoryUnavailable)?;
                Ok(())
            },
        )
        .map_err(|error| run_error_response(error, request_id))?;
    let (run, reused) = match creation {
        RunCreation::Created(run) => (run, false),
        RunCreation::Reused(run) => (run, true),
    };
    let run_id = run.id().clone();

    let events_url = run_events_url(&headers, &run_id);

    if reused {
        let conversation_id = history
            .conversation_for_run(&run_id)
            .map_err(|_| api_error_response(ApiErrorCode::Unavailable, request_id))?;
        return Ok((
            no_store_headers(),
            Json(StartRunResponse {
                run_id,
                events_url,
                conversation_id,
            }),
        ));
    }

    let mut request = request;
    if request.resume.is_none() {
        request.prompt = composer.prompt(&request.prompt);
    }
    tokio::spawn(execute_run_with_history(
        paths,
        selection,
        run,
        request,
        prior_context,
    ));

    Ok((
        no_store_headers(),
        Json(StartRunResponse {
            run_id,
            events_url,
            conversation_id,
        }),
    ))
}

/// Execute a claimed run for either an HTTP submission or the local scheduler.
pub(crate) async fn execute_run(
    paths: orchester_anwendung::OrchesterPaths,
    selection: crate::model_selection::ModelSelection,
    run: RunHandle,
    request: StartRunRequest,
) {
    let history = orchester_anwendung::SessionHistory::for_paths(&paths);
    let prior_context = match request.conversation_id.as_deref() {
        Some(id) => match history.conversation_context(id) {
            Ok(records) => records,
            Err(_) => {
                complete_unfinished_run(&run, Some("history_unavailable")).await;
                return;
            }
        },
        None => Vec::new(),
    };
    let initial = match run.initial_event() {
        Ok(event) => event,
        Err(_) => {
            complete_unfinished_run(&run, Some("history_unavailable")).await;
            return;
        }
    };
    if history
        .begin_conversation_with_event(
            &initial,
            request.conversation_id.as_deref(),
            prior_context.len(),
        )
        .is_err()
    {
        complete_unfinished_run(&run, Some("history_unavailable")).await;
        return;
    }
    execute_run_with_history(paths, selection, run, request, prior_context).await;
}

async fn execute_run_with_history(
    paths: orchester_anwendung::OrchesterPaths,
    selection: crate::model_selection::ModelSelection,
    run: RunHandle,
    request: StartRunRequest,
    prior_context: Vec<orchester_laufzeit::harness::transcript::TranscriptRecord>,
) {
    let run_id = run.id().clone();
    let cancel = run.cancellation_token();
    let StartRunRequest { prompt, resume, .. } = request;
    let (sink, receiver) = RegistryRunSink::channel();
    let drain = drain_run_events(run.clone(), run_id.clone(), receiver);

    let run_task = async move {
        // One host per run: the host's run entry points take `&mut self`, and
        // sharing it would serialise runs the registry keeps independent.
        let mut host = SelfAgentHost::for_paths(&paths);
        // The model is the one the reader chose in the browser, applied to
        // this host exactly as the catalog route applies it to describe the
        // next run. A choice the configuration no longer supports stops the
        // run with an error event rather than quietly running on another
        // model.
        if selection.apply(&mut host).is_err() {
            return Some("model_unavailable");
        }
        let assigned_run_id = orchester_anwendung::SessionHistory::planned_runtime_run_id(&run_id);
        let sink: Arc<dyn RunEventSink> = Arc::new(sink);
        let result = match resume {
            Some(handle) => {
                host.resume_narrated(&handle, cancel, None, Some(sink))
                    .await
            }
            None => {
                host.submit_conversation_narrated(
                    prompt,
                    prior_context,
                    Some(assigned_run_id),
                    cancel,
                    None,
                    Some(sink),
                )
                .await
            }
        };
        // `sink` is dropped here, which closes the channel and lets the
        // drain task finish; `run_task` owns it so this is the only copy.
        result.err().map(|_| "runtime")
    };
    // Consume narration while execution is in progress, not after it has
    // finished. Once all retained events have drained, a startup failure or
    // a missing terminal event must not leave the browser waiting forever.
    let (failure_code, ()) = tokio::join!(run_task, drain);
    complete_unfinished_run(&run, failure_code).await;
}

async fn complete_unfinished_run(run: &RunHandle, failure_code: Option<&str>) {
    let Ok(summary) = run.summary().await else {
        return;
    };
    if summary.stopped {
        return;
    }
    let Ok(snapshot) = run.snapshot().await else {
        return;
    };
    if !snapshot
        .events
        .iter()
        .any(|event| matches!(event.kind, UiEventKind::Error { .. }))
    {
        // Configuration and runtime errors can contain paths, provider details
        // or credentials. The public fallback never formats the source error.
        let code = failure_code.unwrap_or("runtime");
        let message = if code == "model_unavailable" {
            "The selected model is unavailable."
        } else {
            "The run could not be completed."
        };
        let _ = run
            .append(UiEventKind::Error {
                code: code.to_owned(),
                message: message.to_owned(),
            })
            .await;
    }
    let _ = run
        .append(UiEventKind::RunStopped {
            reason: StopReason::Failed,
        })
        .await;
}

/// The absolute socket URL for a run, built from the request the client made so
/// it never has to reconstruct a URL from a port it guessed.
pub(crate) fn run_events_url(headers: &HeaderMap, run_id: &RunId) -> String {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .unwrap_or("127.0.0.1");
    format!("ws://{host}/api/v1/runs/{}/events", run_id.0)
}

fn parse_idempotency_key(headers: &HeaderMap) -> Result<Option<String>, ()> {
    let mut values = headers.get_all("idempotency-key").iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(());
    }
    let value = value.to_str().map_err(|_| ())?;
    if value.is_empty()
        || value.len() > RUN_IDEMPOTENCY_KEY_MAX_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(());
    }
    Ok(Some(value.to_owned()))
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

#[derive(Debug, Deserialize)]
pub(crate) struct RunEventQuery {
    #[serde(default)]
    after_sequence: Option<u64>,
}

/// Replay the retained window from the client cursor, then stream live frames.
///
/// Replay and subscription happen before any frame is written so a run cannot
/// slip an event between the replay response and the live subscription.
pub(crate) async fn run_events_socket_handler(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<RunEventQuery>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let request_id = request_id_from_headers(&headers);
    let after_sequence = query.after_sequence.unwrap_or(0);
    let run = match registered_run(&context, run_id, request_id) {
        Ok(run) => run,
        Err(error) => return error.into_response(),
    };
    let receiver = run.subscribe();
    upgrade.on_upgrade(move |socket| stream_run_events(socket, run, receiver, after_sequence))
}

async fn stream_run_events(
    mut socket: WebSocket,
    run: RunHandle,
    mut receiver: tokio::sync::broadcast::Receiver<RunStreamFrameDto>,
    after_sequence: u64,
) {
    let replay = match run.replay(after_sequence, None).await {
        Ok(replay) => replay,
        Err(RunRegistryError::RetentionExceeded {
            requested_after_sequence,
            oldest_sequence,
            latest_sequence,
        }) => {
            let frame = RunStreamFrameDto::ResyncRequired {
                run_id: run.id().clone(),
                requested_after_sequence,
                oldest_sequence,
                latest_sequence,
                reason: crate::run_contract::ResyncReason::RetentionExceeded,
            };
            let _ = send_run_frame(&mut socket, frame).await;
            return;
        }
        Err(_) => return,
    };

    let mut latest_sequence = after_sequence;
    for event in replay.events {
        latest_sequence = event.sequence;
        if send_run_frame(&mut socket, RunStreamFrameDto::Event { event })
            .await
            .is_err()
        {
            return;
        }
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(payload))) => {
                    if socket.send(Message::Pong(payload)).await.is_err() {
                        break;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) => break,
            },
            frame = receiver.recv() => match frame {
                Ok(RunStreamFrameDto::Event { event }) => {
                    if event.sequence <= latest_sequence {
                        continue;
                    }
                    latest_sequence = event.sequence;
                    if send_run_frame(&mut socket, RunStreamFrameDto::Event { event })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(frame @ RunStreamFrameDto::ResyncRequired { .. }) => {
                    if send_run_frame(&mut socket, frame).await.is_err() {
                        break;
                    }
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let Ok(snapshot) = run.snapshot().await else {
                        break;
                    };
                    let frame = RunStreamFrameDto::ResyncRequired {
                        run_id: run.id().clone(),
                        requested_after_sequence: latest_sequence,
                        oldest_sequence: snapshot.oldest_sequence,
                        latest_sequence: snapshot.latest_sequence,
                        reason: crate::run_contract::ResyncReason::RetentionExceeded,
                    };
                    let _ = send_run_frame(&mut socket, frame).await;
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            },
        }
    }
}

async fn send_run_frame(socket: &mut WebSocket, frame: RunStreamFrameDto) -> Result<(), ()> {
    let text = serde_json::to_string(&frame).map_err(|_| ())?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ())
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
        RunRegistryError::IdempotencyConflict => ApiErrorCode::Conflict,
        RunRegistryError::HistoryUnavailable => ApiErrorCode::Unavailable,
        RunRegistryError::Entropy
        | RunRegistryError::InvalidEvent(_)
        | RunRegistryError::LockPoisoned => ApiErrorCode::Internal,
    };
    api_error_response(code, request_id)
}

#[cfg(test)]
mod tests {

    use std::net::SocketAddr;

    use axum::{
        body::{to_bytes, Body},
        http::{header, Request, StatusCode},
        response::Response,
    };
    use futures::StreamExt;
    use orchester_protokoll::UiEventKind;
    use serde_json::Value;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
    use tower::ServiceExt;

    use super::*;
    use crate::{app_router, ServerControl};

    #[tokio::test]
    async fn failed_journal_write_does_not_advance_sequence_or_make_a_stop_unretryable() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("orchester-journal-rollback-{nonce}"));
        std::fs::create_dir_all(base.join("workspace")).unwrap();
        let paths =
            orchester_anwendung::OrchesterPaths::new(base.join("home"), base.join("workspace"));
        let context = ServerContext::new(Some(paths.clone()), ServerControl::new());
        let RunCreation::Created(run) = context
            .runs()
            .create_or_reuse(
                None,
                [0; 32],
                UiEventKind::UserMessage {
                    text: "saved question".into(),
                },
            )
            .unwrap()
        else {
            panic!("new run")
        };
        let history = context.session_history().unwrap();
        let id = history
            .begin_conversation_turn(run.id(), None, "saved question", None, 0)
            .unwrap();
        run.persist_initial_history().unwrap();
        let mut subscriber = run.subscribe();
        let before = run.snapshot().await.unwrap();
        let connection = rusqlite::Connection::open(
            paths
                .run_database()
                .with_file_name("conversation-history.db"),
        )
        .unwrap();
        connection.execute_batch("CREATE TRIGGER reject_history BEFORE INSERT ON conversation_ui_events BEGIN SELECT RAISE(ABORT, 'injected storage failure'); END;").unwrap();
        let RunCreation::Created(unpublished) = context
            .runs()
            .create_or_reuse(
                None,
                [1; 32],
                UiEventKind::UserMessage {
                    text: "must roll back together".into(),
                },
            )
            .unwrap()
        else {
            panic!("new run")
        };
        assert!(history
            .begin_conversation_with_event(&unpublished.initial_event().unwrap(), None, 0)
            .is_err());
        assert_eq!(
            history.page(None, 20).unwrap().items.len(),
            1,
            "a failed initial envelope must not leave a second conversation"
        );
        assert!(history.conversation_for_run(unpublished.id()).is_err());
        assert_eq!(
            run.append(UiEventKind::RunStopped {
                reason: StopReason::Failed
            })
            .await
            .unwrap_err(),
            RunRegistryError::HistoryUnavailable
        );
        assert_eq!(run.snapshot().await.unwrap(), before);
        assert!(!run.summary().await.unwrap().stopped);
        assert!(subscriber.try_recv().is_err());
        assert_eq!(
            history.detail(&id).unwrap().events.len(),
            1,
            "terminal metadata must roll back with the envelope"
        );
        connection
            .execute_batch("DROP TRIGGER reject_history;")
            .unwrap();
        let event = run
            .append(UiEventKind::RunStopped {
                reason: StopReason::Failed,
            })
            .await
            .unwrap();
        assert_eq!(event.sequence, 2);
        assert!(run.summary().await.unwrap().stopped);
        assert_eq!(history.detail(&id).unwrap().events.len(), 2);
        drop(connection);
        drop(run);
        drop(context);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[tokio::test]
    async fn long_conversation_keeps_all_events_on_reload_and_server_restart() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("orchester-durable-ui-{nonce}"));
        std::fs::create_dir_all(base.join("workspace")).unwrap();
        let paths =
            orchester_anwendung::OrchesterPaths::new(base.join("home"), base.join("workspace"));
        let context = ServerContext::new(Some(paths.clone()), ServerControl::new());
        let RunCreation::Created(run) = context
            .runs()
            .create_or_reuse(
                None,
                [0; 32],
                UiEventKind::UserMessage {
                    text: "keep the entire conversation".into(),
                },
            )
            .unwrap()
        else {
            panic!("new run")
        };
        let history = context.session_history().unwrap();
        let conversation_id = history
            .begin_conversation_turn(run.id(), None, "keep the entire conversation", None, 0)
            .unwrap();
        run.persist_initial_history().unwrap();
        for index in 0..300 {
            run.append(UiEventKind::Reasoning {
                text: format!("reasoning {index}"),
            })
            .await
            .unwrap();
        }
        let snapshot = run.snapshot().await.unwrap();
        assert_eq!(
            snapshot.events.len(),
            301,
            "reload snapshot must include events evicted from the socket ring"
        );
        assert_eq!(snapshot.oldest_sequence, 1);
        assert!(matches!(
            snapshot.events[0].kind,
            UiEventKind::UserMessage { .. }
        ));
        let response = app_router(context.clone())
            .oneshot(
                Request::get(format!("/api/v1/sessions/{conversation_id}"))
                    .header(header::HOST, "127.0.0.1:9090")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let detail = json_body(response).await;
        assert_eq!(detail["active_run"]["run_id"], run.id().0);
        assert_eq!(detail["active_run"]["conversation_id"], conversation_id);
        assert!(detail["active_run"]["events_url"]
            .as_str()
            .unwrap()
            .starts_with("ws://127.0.0.1:9090/"));
        run.append(UiEventKind::Message {
            text: "The complete answer".into(),
        })
        .await
        .unwrap();
        run.append(UiEventKind::RunStopped {
            reason: StopReason::Succeeded,
        })
        .await
        .unwrap();
        let live_ids = run
            .snapshot()
            .await
            .unwrap()
            .events
            .into_iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>();
        drop(run);
        drop(context);
        let restarted = ServerContext::new(Some(paths), ServerControl::new());
        let detail = restarted
            .session_history()
            .unwrap()
            .detail(&conversation_id)
            .unwrap();
        assert_eq!(detail.events.len(), 303);
        assert_eq!(
            detail
                .events
                .iter()
                .map(|event| event.event_id.clone())
                .collect::<Vec<_>>(),
            live_ids
        );
        let response = app_router(restarted)
            .oneshot(
                Request::get(format!("/api/v1/sessions/{conversation_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(json_body(response).await.get("active_run").is_none());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[tokio::test]
    async fn failed_http_submission_is_saved_and_can_continue_after_server_restart() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("orchester-failed-ui-{nonce}"));
        std::fs::create_dir_all(base.join("workspace")).unwrap();
        let paths =
            orchester_anwendung::OrchesterPaths::new(base.join("home"), base.join("workspace"));
        let context = ServerContext::new(Some(paths.clone()), ServerControl::new());
        let (_, Json(started)) = start_run_handler(
            State(context.clone()),
            HeaderMap::new(),
            Ok(Json(StartRunRequest {
                prompt: "keep my failed request".into(),
                resume: None,
                conversation_id: None,
            })),
        )
        .await
        .unwrap();
        let run = context.runs().get(&started.run_id).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !run.summary().await.unwrap().stopped {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let restarted = ServerContext::new(Some(paths), ServerControl::new());
        let detail = restarted
            .session_history()
            .unwrap()
            .detail(&started.conversation_id)
            .unwrap();
        assert_eq!(detail.prompt, "keep my failed request");
        assert!(detail
            .events
            .iter()
            .any(|event| matches!(event.kind, UiEventKind::Error { .. })));
        assert!(detail.events.iter().any(|event| matches!(
            event.kind,
            UiEventKind::RunStopped {
                reason: StopReason::Failed
            }
        )));
        assert_eq!(
            restarted
                .session_history()
                .unwrap()
                .conversation_context(&started.conversation_id)
                .unwrap(),
            vec![
                orchester_laufzeit::harness::transcript::TranscriptRecord::user(
                    "keep my failed request"
                )
            ]
        );
        drop(run);
        drop(context);
        drop(restarted);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[tokio::test]
    async fn dropping_a_request_waiting_for_model_selection_does_not_reserve_its_key() {
        let base = std::env::temp_dir().join("orchester-unpublished-run-test");
        let context = ServerContext::new(
            Some(orchester_anwendung::OrchesterPaths::new(
                base.join("home"),
                &base,
            )),
            ServerControl::new(),
        );
        let guard = context.model_selection().edit().await;
        let mut headers = HeaderMap::new();
        headers.insert(
            "idempotency-key",
            "aborted-before-publication".parse().unwrap(),
        );
        {
            let pending = start_run_handler(
                State(context.clone()),
                headers.clone(),
                Ok(Json(StartRunRequest {
                    prompt: "first".into(),
                    resume: None,
                    conversation_id: None,
                })),
            );
            tokio::pin!(pending);
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(20), pending)
                    .await
                    .is_err()
            );
        }
        drop(guard);
        // A changed request is accepted under the same key: the dropped first
        // HTTP future never exposed a run or took ownership of the submission.
        let (_, Json(response)) = start_run_handler(
            State(context),
            headers,
            Ok(Json(StartRunRequest {
                prompt: "second".into(),
                resume: None,
                conversation_id: None,
            })),
        )
        .await
        .expect("new request after the original future was dropped");
        assert!(!response.run_id.0.is_empty());
    }

    #[tokio::test]
    async fn unfinished_failure_keeps_one_error_and_broadcasts_one_terminal_frame() {
        let registry = crate::run_registry::RunRegistry::new(8);
        let run = registry.create().unwrap();
        run.append(UiEventKind::UserMessage {
            text: "asked".into(),
        })
        .await
        .unwrap();
        run.append(UiEventKind::Error {
            code: "runtime".into(),
            message: "failed".into(),
        })
        .await
        .unwrap();
        let mut frames = run.subscribe();
        complete_unfinished_run(&run, Some("runtime")).await;
        complete_unfinished_run(&run, Some("runtime")).await;
        let snapshot = run.snapshot().await.unwrap();
        assert_eq!(snapshot.state, crate::run_contract::RunStateDto::Failed);
        assert_eq!(snapshot.events.len(), 3);
        assert!(run.summary().await.unwrap().stopped);
        assert!(matches!(
            frames.try_recv(),
            Ok(RunStreamFrameDto::Event {
                event: orchester_protokoll::UiEventEnvelope {
                    kind: UiEventKind::RunStopped {
                        reason: StopReason::Failed
                    },
                    ..
                }
            })
        ));
        assert!(frames.try_recv().is_err());
    }

    #[tokio::test]
    async fn the_failure_fallback_preserves_a_cancelled_run() {
        let registry = crate::run_registry::RunRegistry::new(8);
        let run = registry.create().unwrap();
        run.cancel().await.unwrap();
        let before = run.snapshot().await.unwrap();
        complete_unfinished_run(&run, Some("model_unavailable")).await;
        assert_eq!(run.snapshot().await.unwrap(), before);
    }

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

    #[tokio::test]
    async fn event_socket_replays_from_the_cursor_then_streams_live_events() {
        let context = ServerContext::new(None, ServerControl::new());
        let run = context.runs().create().expect("create run");
        run.append(UiEventKind::Message { text: "one".into() })
            .await
            .expect("append first event");
        run.append(UiEventKind::Message { text: "two".into() })
            .await
            .expect("append second event");
        let address = serve(context.clone()).await;

        let (mut socket, _) = connect_async(format!(
            "ws://{address}/api/v1/runs/{}/events?after_sequence=1",
            run.id().0
        ))
        .await
        .expect("connect run socket");
        let replayed = read_socket_json(&mut socket).await;
        assert_eq!(replayed["type"], "event");
        assert_eq!(replayed["event"]["sequence"], 2);
        assert_eq!(replayed["event"]["kind"]["text"], "two");

        run.append(UiEventKind::Message {
            text: "three".into(),
        })
        .await
        .expect("append live event");
        let live = read_socket_json(&mut socket).await;
        assert_eq!(live["type"], "event");
        assert_eq!(live["event"]["sequence"], 3);
        assert_eq!(live["event"]["kind"]["text"], "three");

        let _ = socket.close(None).await;
    }

    #[tokio::test]
    async fn event_socket_requests_resync_for_an_evicted_cursor() {
        let context = ServerContext::new(None, ServerControl::new());
        let run = context.runs().create().expect("create run");
        for index in 0..257 {
            run.append(UiEventKind::Message {
                text: format!("event {index}"),
            })
            .await
            .expect("append retained event");
        }
        let address = serve(context.clone()).await;

        let (mut socket, _) = connect_async(format!(
            "ws://{address}/api/v1/runs/{}/events?after_sequence=0",
            run.id().0
        ))
        .await
        .expect("connect run socket");
        let frame = read_socket_json(&mut socket).await;
        assert_eq!(frame["type"], "resync_required");
        assert_eq!(frame["run_id"], run.id().0);
        assert_eq!(frame["requested_after_sequence"], 0);
        assert_eq!(frame["oldest_sequence"], 2);
        assert_eq!(frame["latest_sequence"], 257);
        assert_eq!(frame["reason"], "retention_exceeded");

        let _ = socket.close(None).await;
    }

    #[tokio::test]
    async fn event_socket_reports_an_unknown_run_as_not_found() {
        let context = ServerContext::new(None, ServerControl::new());
        let address = serve(context.clone()).await;

        let error = connect_async(format!("ws://{address}/api/v1/runs/run-missing/events"))
            .await
            .expect_err("unknown runs must not upgrade");
        assert!(
            error.to_string().contains("404") || format!("{error:?}").contains("404"),
            "{error:?}"
        );
    }

    async fn serve(context: ServerContext) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind socket");
        let address = listener.local_addr().expect("socket address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app_router(context)).await;
        });
        address
    }

    async fn read_socket_json<S>(stream: &mut tokio_tungstenite::WebSocketStream<S>) -> Value
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        match stream
            .next()
            .await
            .expect("socket frame")
            .expect("socket message")
        {
            TungsteniteMessage::Text(text) => serde_json::from_str(&text).expect("socket JSON"),
            other => panic!("expected text frame, got {other:?}"),
        }
    }

    async fn json_body(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        serde_json::from_slice::<Value>(&body).expect("response JSON")
    }
}
