use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    health::no_store_headers,
    run_contract::{RunStateDto, StartRunRequest},
    run_registry::RunCreation,
    schedule_store::{self, Claim, Schedule, ScheduleInput, ScheduleRun, ScheduleStore},
    ServerContext,
};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use orchester_anwendung::OrchesterPaths;
use orchester_protokoll::{StopReason, UiEventKind};
use serde::Serialize;
use std::{sync::atomic::Ordering, time::Duration};

async fn storage<T: Send + 'static>(
    paths: OrchesterPaths,
    action: impl FnOnce(&mut ScheduleStore) -> schedule_store::Result<T> + Send + 'static,
) -> schedule_store::Result<T> {
    tokio::task::spawn_blocking(move || action(&mut ScheduleStore::open(&paths)?))
        .await
        .map_err(|_| ApiErrorCode::Internal)?
}

#[derive(Serialize)]
pub(crate) struct ScheduleCatalog {
    schema_version: u8,
    schedules: Vec<Schedule>,
}

pub(crate) async fn list(
    State(context): State<ServerContext>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<ScheduleCatalog>), ApiErrorResponse> {
    let fail = |code| api_error_response(code, request_id_from_headers(&headers));
    let paths = context
        .paths()
        .cloned()
        .ok_or_else(|| fail(ApiErrorCode::Unavailable))?;
    let schedules = storage(paths, |store| store.list()).await.map_err(fail)?;
    Ok((
        no_store_headers(),
        Json(ScheduleCatalog {
            schema_version: 1,
            schedules,
        }),
    ))
}

async fn save(
    context: ServerContext,
    headers: &HeaderMap,
    id: Option<String>,
    request: Result<Json<ScheduleInput>, JsonRejection>,
) -> Result<Schedule, ApiErrorResponse> {
    let fail = |code| api_error_response(code, request_id_from_headers(headers));
    let Json(input) = request.map_err(|_| fail(ApiErrorCode::BadRequest))?;
    input.validate().map_err(fail)?;
    let paths = context
        .paths()
        .cloned()
        .ok_or_else(|| fail(ApiErrorCode::Unavailable))?;
    let selection = context.model_selection().read().await;
    storage(paths, move |store| {
        store.save(id.as_deref(), input, selection)
    })
    .await
    .map_err(fail)
}

pub(crate) async fn create(
    State(context): State<ServerContext>,
    headers: HeaderMap,
    request: Result<Json<ScheduleInput>, JsonRejection>,
) -> Result<(StatusCode, HeaderMap, Json<Schedule>), ApiErrorResponse> {
    Ok((
        StatusCode::CREATED,
        no_store_headers(),
        Json(save(context, &headers, None, request).await?),
    ))
}

pub(crate) async fn update(
    State(context): State<ServerContext>,
    Path(id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<ScheduleInput>, JsonRejection>,
) -> Result<(HeaderMap, Json<Schedule>), ApiErrorResponse> {
    Ok((
        no_store_headers(),
        Json(save(context, &headers, Some(id), request).await?),
    ))
}

pub(crate) async fn remove(
    State(context): State<ServerContext>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, HeaderMap), ApiErrorResponse> {
    let fail = |code| api_error_response(code, request_id_from_headers(&headers));
    let paths = context
        .paths()
        .cloned()
        .ok_or_else(|| fail(ApiErrorCode::Unavailable))?;
    storage(paths, move |store| store.delete(&id))
        .await
        .map_err(fail)?;
    Ok((StatusCode::NO_CONTENT, no_store_headers()))
}

pub(crate) async fn run_now(
    State(context): State<ServerContext>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, HeaderMap, Json<ScheduleRun>), ApiErrorResponse> {
    let fail = |code| api_error_response(code, request_id_from_headers(&headers));
    let paths = context
        .paths()
        .cloned()
        .ok_or_else(|| fail(ApiErrorCode::Unavailable))?;
    let owner = context.scheduler_owner.clone();
    // Once claimed, a detached task owns the handoff even if the browser disconnects.
    let task = tokio::spawn(async move {
        let claim = storage(paths, move |store| {
            store.claim(Some(&id), &owner, schedule_store::now())
        })
        .await?
        .ok_or(ApiErrorCode::NotFound)?;
        launch(context, claim)
    });
    let result = task
        .await
        .map_err(|_| fail(ApiErrorCode::Internal))?
        .map_err(fail)?;
    Ok((StatusCode::ACCEPTED, no_store_headers(), Json(result)))
}

fn launch(context: ServerContext, claim: Claim) -> schedule_store::Result<ScheduleRun> {
    let request = StartRunRequest {
        prompt: claim.task.input.prompt,
        resume: None,
    };
    let creation = context
        .runs()
        .create_or_reuse(
            None,
            request.fingerprint(),
            UiEventKind::UserMessage {
                text: request.prompt.clone(),
            },
        )
        .map_err(|_| ApiErrorCode::Unavailable)?;
    let run = match creation {
        RunCreation::Created(run) | RunCreation::Reused(run) => run,
    };
    let mut record = claim.execution;
    let run_id = run.id().0.clone();
    record.run_id = Some(run_id.clone());
    let returned = record.clone();
    let paths = context.paths().cloned().ok_or(ApiErrorCode::Unavailable)?;
    tokio::spawn(async move {
        let owner = context.scheduler_owner.clone();
        let id = record.id.clone();
        let bind_owner = owner.clone();
        let bind = storage(paths.clone(), move |store| {
            store.bind(&id, &bind_owner, &run_id)
        })
        .await;
        let mut shutdown = context.control().subscribe_shutdown();
        let mut status = "failed";
        let mut error = Some("runtime");
        if bind.is_ok() && !*shutdown.borrow() {
            let cancel = run.cancellation_token();
            let execution =
                crate::run::execute_run(paths.clone(), claim.selection, run.clone(), request);
            tokio::pin!(execution);
            let mut renewal = tokio::time::interval(Duration::from_secs(10));
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown.changed() => { status = "interrupted"; error = Some("interrupted"); cancel.cancel(); break; }
                    _ = &mut execution => {
                        if let Ok(snapshot) = run.snapshot().await {
                            status = match snapshot.state { RunStateDto::Succeeded => "succeeded", RunStateDto::Cancelled => "cancelled", _ => "failed" };
                            error = if status == "failed" { Some("runtime") } else { None };
                        }
                        break;
                    }
                    _ = renewal.tick() => {
                        let renew_owner = owner.clone();
                        if storage(paths.clone(), move |store| store.heartbeat(&renew_owner)).await.is_err() {
                            status = "interrupted"; error = Some("storage_unavailable"); cancel.cancel(); break;
                        }
                    }
                }
            }
            if status == "interrupted" {
                let _ = tokio::time::timeout(Duration::from_secs(5), &mut execution).await;
            }
        } else {
            let _ = run
                .append(UiEventKind::Error {
                    code: "schedule_unavailable".into(),
                    message: "The scheduled run could not start.".into(),
                })
                .await;
            let _ = run
                .append(UiEventKind::RunStopped {
                    reason: StopReason::Failed,
                })
                .await;
        }
        let _ = storage(paths, move |store| {
            store.finish(&record.id, &owner, status, error)
        })
        .await;
    });
    Ok(returned)
}

impl ServerContext {
    /// The desktop owns this loop; schedules run while the app is open or in its tray.
    pub fn start_scheduler(&self) -> bool {
        let Some(paths) = self.paths().cloned() else {
            return false;
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return false;
        };
        if self
            .scheduler_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        let context = self.clone();
        runtime.spawn(async move {
            let mut shutdown = context.control().subscribe_shutdown();
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                if *shutdown.borrow() { break; }
                tokio::select! {
                    biased;
                    _ = shutdown.changed() => break,
                    _ = ticker.tick() => {
                        for _ in 0..4 {
                            let owner = context.scheduler_owner.clone();
                            match storage(paths.clone(), move |store| store.claim(None, &owner, schedule_store::now())).await {
                                Ok(Some(claim)) => { let _ = launch(context.clone(), claim); }
                                _ => break,
                            }
                        }
                    }
                }
            }
        });
        true
    }
}
