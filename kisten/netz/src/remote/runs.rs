use super::*;
use axum::extract::{Path, Query};
use futures::StreamExt;
pub(super) async fn list(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Query(q): Query<ProjectQuery>,
) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    if q.project_id.is_some() {
        s.project(q.project_id.as_deref())?;
    }
    let mut runs: Vec<_> =
        s.0.runs
            .lock()
            .unwrap()
            .values()
            .filter(|run| {
                q.project_id
                    .as_deref()
                    .is_none_or(|id| run.summary["project_id"] == id)
            })
            .map(|run| run.summary.clone())
            .collect();
    runs.sort_by(|a, b| {
        b["created_at"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["created_at"].as_u64().unwrap_or(0))
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    Ok(Json(json!({"runs":runs})))
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Start {
    agent: String,
    project_id: Option<String>,
    prompt: String,
    resume: Option<String>,
    idempotency_key: Option<String>,
}
pub(super) async fn start(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Json(p): Json<Start>,
) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    if p.prompt.trim().is_empty()
        || p.prompt.len() > 32000
        || p.resume
            .as_ref()
            .is_some_and(|r| r.trim().is_empty() || r.len() > 256)
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let fingerprint = hash(&serde_json::to_string(&p).map_err(|_| StatusCode::BAD_REQUEST)?);
    if p.idempotency_key
        .as_ref()
        .is_some_and(|k| k.is_empty() || k.len() > 128)
    {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let project = s.project(p.project_id.as_deref())?;
    s.validate_project(&project)?;
    let registry = s.project_registry(&project);
    let adapter = registry.get(&p.agent).ok_or(StatusCode::NOT_FOUND)?;
    if p.resume.is_some() && !adapter.capabilities().supports_resume {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let id = random().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let summary = json!({"id":id,"agent":p.agent,"project_id":project.id,"workspace":project.path,"prompt":p.prompt,"state":"running","created_at":now()});
    let cancel = CancellationToken::new();
    {
        let mut runs = s.0.runs.lock().unwrap();
        if s.0.stopping.load(Ordering::SeqCst) {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        if let Some(key) = &p.idempotency_key {
            if let Some(existing) = runs.values().find(|r| {
                r.idempotency_key.as_ref() == Some(key) && r.summary["project_id"] == project.id
            }) {
                #[derive(Serialize)]
                struct LegacyStart<'a> {
                    agent: &'a str,
                    prompt: &'a str,
                    resume: &'a Option<String>,
                    idempotency_key: &'a Option<String>,
                }
                let legacy = hash(
                    &serde_json::to_string(&LegacyStart {
                        agent: &p.agent,
                        prompt: &p.prompt,
                        resume: &p.resume,
                        idempotency_key: &p.idempotency_key,
                    })
                    .map_err(|_| StatusCode::BAD_REQUEST)?,
                );
                return if existing.fingerprint == fingerprint
                    || (p.project_id.is_none() && existing.fingerprint == legacy)
                {
                    Ok(Json(existing.summary.clone()))
                } else {
                    Err(StatusCode::CONFLICT)
                };
            }
        }
        if let Some(session) = &p.resume {
            if !runs.values().any(|run| {
                run.summary["project_id"] == project.id
                    && run.summary["agent"] == p.agent
                    && run.summary["session_id"] == session.as_str()
                    && run.summary["state"] != "running"
            }) {
                return Err(StatusCode::UNPROCESSABLE_ENTITY);
            }
        }
        if runs.len() >= 128
            || runs
                .values()
                .filter(|r| r.summary["state"] == "running")
                .count()
                >= 4
        {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        runs.insert(
            id.clone(),
            Run {
                summary: summary.clone(),
                bytes: 0,
                events: vec![],
                cancel: cancel.clone(),
                fingerprint,
                idempotency_key: p.idempotency_key.clone(),
            },
        );
        if s.save_run(&id, runs.get(&id).unwrap()).is_err() {
            runs.remove(&id);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    let mut task = orchester_protokoll::Task::new(p.prompt, project.path.clone());
    task.resume = p.resume;
    tokio::spawn(async move {
        let execution = async {
            let conductor = orchester_laufzeit::Conductor::new(registry);
            let mut session = orchester_laufzeit::Session::new();
            let mut stream = conductor
                .run(&p.agent, task)
                .await
                .map_err(|_| "agent_start_failed")?;
            while let Some(event) = stream.next().await {
                let event = event.map_err(|error| match error {
                    orchester_vertrag::AdapterError::OutputLimit => "output_limit_exceeded",
                    _ => "agent_stream_failed",
                })?;
                session.observe(&event);
                let failed = matches!(event, orchester_protokoll::Event::Error { .. });
                {
                    let mut runs = s.0.runs.lock().unwrap();
                    let r = runs.get_mut(&id).unwrap();
                    if r.events.len() >= 10000 {
                        return Err("event_limit_exceeded");
                    }
                    let bytes = serde_json::to_vec(&event)
                        .map_err(|_| "invalid_event")?
                        .len();
                    if bytes > 1024 * 1024 || r.bytes + bytes > 16 * 1024 * 1024 {
                        return Err("output_limit_exceeded");
                    }
                    r.bytes += bytes;
                    r.summary["session_id"] = json!(session.session_id());
                    r.events.push(json!({"seq":r.events.len()+1,"event":event}));
                    s.save_run(&id, runs.get(&id).unwrap())
                        .map_err(|_| "storage_failed")?;
                }
                if failed {
                    return Err("agent_reported_error");
                }
            }
            let result = session.finish();
            if result.outcome == orchester_protokoll::Outcome::Failed {
                return Err("agent_reported_error");
            }
            Ok::<_, &str>(result.outcome)
        };
        let (state, error_code) = tokio::select! {
            biased;
            _=cancel.cancelled()=>("cancelled",None),
            result=execution=>match result {
                Ok(orchester_protokoll::Outcome::Cancelled)=>("cancelled",None),
                Ok(_)=>("completed",None),
                Err(code)=>("failed",Some(code)),
            }
        };
        let mut runs = s.0.runs.lock().unwrap();
        let run = runs.get_mut(&id).unwrap();
        run.summary["state"] = json!(state);
        if let Some(code) = error_code {
            run.summary["error_code"] = json!(code);
        }
        if let Err(error) = s.save_run(&id, runs.get(&id).unwrap()) {
            tracing::error!(%error,"remote final state persistence failed");
            let run = runs.get_mut(&id).unwrap();
            run.summary["state"] = json!("failed");
            run.summary["error_code"] = json!("storage_failed");
            // Persist the failure when the storage error was specific to the
            // terminal transition, without claiming success if retry also fails.
            if let Err(error) = s.save_run(&id, runs.get(&id).unwrap()) {
                tracing::error!(%error,"remote storage failure state persistence failed");
            }
        }
    });
    Ok(Json(summary))
}
pub(super) async fn snapshot(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    Ok(Json(
        s.0.runs
            .lock()
            .unwrap()
            .get(&id)
            .ok_or(StatusCode::NOT_FOUND)?
            .summary
            .clone(),
    ))
}
#[derive(Deserialize)]
pub(super) struct Cursor {
    #[serde(default)]
    after: usize,
}
pub(super) async fn events(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Cursor>,
) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    let runs = s.0.runs.lock().unwrap();
    let r = runs.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if q.after > r.events.len() {
        return Err(StatusCode::CONFLICT);
    }
    let page: Vec<_> = r.events.iter().skip(q.after).take(200).cloned().collect();
    Ok(Json(
        json!({"next_cursor":q.after+page.len(),"events":page,"state":r.summary["state"]}),
    ))
}
pub(super) async fn cancel(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    let runs = s.0.runs.lock().unwrap();
    let r = runs.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    r.cancel.cancel();
    Ok(Json(r.summary.clone()))
}

/// Deletion is explicit: clients end the corresponding idempotency retention window.
pub(super) async fn remove(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    s.authorize(&h)?;
    let mut runs = s.0.runs.lock().unwrap();
    let run = runs.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    if run.summary["state"] == "running" {
        return Err(StatusCode::CONFLICT);
    }
    s.0.database
        .lock()
        .unwrap()
        .execute(
            "DELETE FROM remote_runs WHERE workspace=?1 AND id=?2",
            rusqlite::params![run.summary["workspace"].as_str().unwrap_or_default(), id],
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    runs.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
