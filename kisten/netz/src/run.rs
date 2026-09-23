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
use orchester_protokoll::{RunId, UiEventKind};
use serde::Deserialize;

use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    health::no_store_headers,
    run_bridge::{drain_run_events, RegistryRunSink},
    run_contract::{
        RunReplayRequestDto, RunReplayResponseDto, RunSnapshotDto, RunStreamFrameDto,
        RunSummaryDto, StartRunRequest, StartRunResponse,
    },
    run_registry::{RunHandle, RunRegistryError},
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

    // Without paths there is no configuration to run: the same availability
    // answer this route gave before a runtime existed.
    let paths = context
        .paths()
        .cloned()
        .ok_or_else(|| api_error_response(ApiErrorCode::Unavailable, request_id))?;

    let run = context
        .runs()
        .create()
        .map_err(|error| run_error_response(error, request_id))?;
    let run_id = run.id().clone();
    let cancel = run.cancellation_token();
    let events_url = run_events_url(&headers, &run_id);

    let StartRunRequest { prompt, resume } = request;

    // The journal opens with the reader's own turn, not with the runtime's
    // answer. A snapshot a browser replays has to show what was asked - the
    // transcript draws the question as the reader's bubble and the message rail
    // navigates by it - and the runtime never reports it back, because from its
    // side the prompt is the request rather than an event. The text is already
    // bounded and control-character checked by `StartRunRequest::validate`.
    run.append(UiEventKind::UserMessage {
        text: prompt.clone(),
    })
    .await
    .map_err(|error| run_error_response(error, request_id))?;

    let (sink, receiver) = RegistryRunSink::channel();
    let drain = drain_run_events(run.clone(), run_id.clone(), receiver);
    // Read before the spawn, so the choice is the one in force when the request
    // was made rather than whatever a later selection says.
    let selection = context.model_selection().read().await;
    tokio::spawn(async move {
        let drain = drain;
        let run_task = async move {
            // One host per run: the host's run entry points take `&mut self`, and
            // sharing it would serialise runs the registry keeps independent.
            let mut host = SelfAgentHost::for_paths(&paths);
            // The model is the one the reader chose in the browser, applied to
            // this host exactly as the catalog route applies it to describe the
            // next run. A choice the configuration no longer supports stops the
            // run with an error event rather than quietly running on another
            // model.
            if let Err(error) = selection.apply(&mut host) {
                let _ = run
                    .append(UiEventKind::Error {
                        code: "model_unavailable".to_owned(),
                        message: error.to_string(),
                    })
                    .await;
                return;
            }
            let sink: Arc<dyn RunEventSink> = Arc::new(sink);
            let _ = match resume {
                Some(handle) => {
                    host.resume_narrated(&handle, cancel, None, Some(sink))
                        .await
                }
                None => host.submit_narrated(prompt, cancel, None, Some(sink)).await,
            };
            // `sink` is dropped here, which closes the channel and lets the
            // drain task finish; `run_task` owns it so this is the only copy.
        };
        run_task.await;
        drain.await;
    });

    Ok((
        no_store_headers(),
        Json(StartRunResponse { run_id, events_url }),
    ))
}

/// The absolute socket URL for a run, built from the request the client made so
/// it never has to reconstruct a URL from a port it guessed.
fn run_events_url(headers: &HeaderMap, run_id: &RunId) -> String {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .unwrap_or("127.0.0.1");
    format!("ws://{host}/runs/{}/events", run_id.0)
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
