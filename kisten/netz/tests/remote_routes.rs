use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use orchester_anwendung::OrchesterPaths;
use orchester_netz::{RemoteControl, ServerContext, ServerControl};
use serde_json::{json, Value};
use tower::ServiceExt;
fn remote(root: &std::path::Path) -> RemoteControl {
    let paths = OrchesterPaths::new(root.join("home"), root);
    let mut registry = orchester_verzeichnis::Registry::discover(paths.manifest_dir());
    registry.register(std::sync::Arc::new(orchester_adapter::MockAdapter::new()));
    RemoteControl::new(
        ServerContext::new(Some(paths), ServerControl::new()).with_registry(registry),
        root.to_path_buf(),
    )
    .unwrap()
}
async fn request(
    s: &RemoteControl,
    method: &str,
    path: &str,
    token: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut r = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if !token.is_empty() {
        r = r.header("authorization", format!("Bearer {token}"));
    }
    let result = s
        .router()
        .oneshot(r.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = result.status();
    let bytes = to_bytes(result.into_body(), 2 * 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn pair(s: &RemoteControl) -> String {
    let (status, v) = request(
        s,
        "POST",
        "/remote/v1/pair",
        "",
        json!({"code":s.pairing_code(),"device_name":"Phone"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    v["token"].as_str().unwrap().into()
}
#[tokio::test]
async fn pairing_is_one_time_persists_and_can_be_revoked() {
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    assert_eq!(
        request(&s, "GET", "/remote/v1/info", "", Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let code = s.pairing_code();
    let token = pair(&s).await;
    assert_eq!(
        request(
            &s,
            "POST",
            "/remote/v1/pair",
            "",
            json!({"code":code,"device_name":"Second"})
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let host = request(&s, "GET", "/remote/v1/info", &token, Value::Null)
        .await
        .1["host_id"]
        .clone();
    drop(s);
    let s = remote(root.path());
    assert_eq!(
        request(&s, "GET", "/remote/v1/info", &token, Value::Null)
            .await
            .1["host_id"],
        host
    );
    let devices = s.devices();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device_name, "Phone");
    s.revoke_device(&devices[0].device_id).unwrap();
    drop(s);
    let s = remote(root.path());
    assert_eq!(
        request(&s, "GET", "/remote/v1/info", &token, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    s.disable_pairing();
    assert_eq!(
        request(
            &s,
            "POST",
            "/remote/v1/pair",
            "",
            json!({"code":s.pairing_code(),"device_name":"Phone"})
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    s.rotate_pairing_code().unwrap();
    pair(&s).await;
    let bytes = std::fs::read(root.path().join("home/state/remote.db")).unwrap();
    assert!(!bytes.windows(token.len()).any(|w| w == token.as_bytes()));
}
#[tokio::test]
async fn mock_stream_retry_and_restart_history() {
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    let payload = json!({"agent":"mock","prompt":"remote test","idempotency_key":"submission-1"});
    let (status, run) = request(&s, "POST", "/remote/v1/runs", &token, payload.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let id = run["id"].as_str().unwrap();
    let retry = request(&s, "POST", "/remote/v1/runs", &token, payload.clone()).await;
    assert_eq!(retry.1["id"], id);
    let mut conflict = payload;
    conflict["prompt"] = json!("changed");
    assert_eq!(
        request(&s, "POST", "/remote/v1/runs", &token, conflict)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let mut final_state = Value::Null;
    for _ in 0..100 {
        final_state = request(
            &s,
            "GET",
            &format!("/remote/v1/runs/{id}"),
            &token,
            Value::Null,
        )
        .await
        .1;
        if final_state["state"] != "running" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(final_state["state"], "completed");
    assert_eq!(final_state["session_id"], "mock-session");
    let page = request(
        &s,
        "GET",
        &format!("/remote/v1/runs/{id}/events?after=0"),
        &token,
        Value::Null,
    )
    .await
    .1;
    assert!(page["events"].as_array().unwrap().len() > 2);
    let cursor = page["next_cursor"].as_u64().unwrap();
    assert_eq!(
        request(
            &s,
            "GET",
            &format!("/remote/v1/runs/{id}/events?after={cursor}"),
            &token,
            Value::Null
        )
        .await
        .1["events"],
        json!([])
    );
    assert_eq!(
        request(
            &s,
            "GET",
            &format!("/remote/v1/runs/{id}/events?after=999999"),
            &token,
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let id = id.to_owned();
    drop(s);
    let s = remote(root.path());
    assert_eq!(
        request(
            &s,
            "GET",
            &format!("/remote/v1/runs/{id}/events"),
            &token,
            Value::Null
        )
        .await
        .1,
        page
    );
    assert_eq!(
        request(
            &s,
            "POST",
            "/remote/v1/runs",
            &token,
            json!({"agent":"mock","prompt":"test","cwd":"/tmp"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        request(&s, "GET", "/api/v1/bootstrap", &token, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn paired_authority_is_computer_bound_and_attempts_are_bounded() {
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    let paths = OrchesterPaths::new(root.path().join("home"), root.path());
    assert!(RemoteControl::new(
        ServerContext::new(Some(paths), ServerControl::new()),
        root.path().to_path_buf()
    )
    .is_err());
    let token = pair(&s).await;
    s.rotate_pairing_code().unwrap();
    for _ in 0..10 {
        assert_eq!(
            request(
                &s,
                "POST",
                "/remote/v1/pair",
                "",
                json!({"code":"wrong","device_name":"Phone"})
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        request(
            &s,
            "POST",
            "/remote/v1/pair",
            "",
            json!({"code":s.pairing_code(),"device_name":"Phone"})
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    drop(s);
    let other = root.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let paths = OrchesterPaths::new(root.path().join("home"), &other);
    let s =
        RemoteControl::new(ServerContext::new(Some(paths), ServerControl::new()), other).unwrap();
    assert_eq!(
        request(&s, "GET", "/remote/v1/info", &token, Value::Null)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn unfinished_persisted_runs_become_interrupted() {
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    drop(s);
    let db = rusqlite::Connection::open(root.path().join("home/state/remote.db")).unwrap();
    let workspace = root
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let value = json!({"summary":{"id":"unfinished","state":"running"},"events":[{"seq":1,"event":{"type":"turn_started"}}],"fingerprint":"f","idempotency_key":null,"bytes":0});
    db.execute(
        "INSERT INTO remote_runs(workspace,id,value) VALUES (?1,'unfinished',?2)",
        rusqlite::params![workspace, value.to_string()],
    )
    .unwrap();
    drop(db);
    let s = remote(root.path());
    assert_eq!(
        request(&s, "GET", "/remote/v1/runs/unfinished", &token, Value::Null)
            .await
            .1["state"],
        "interrupted"
    );
    assert_eq!(
        request(
            &s,
            "GET",
            "/remote/v1/runs/unfinished/events",
            &token,
            Value::Null
        )
        .await
        .1["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn cancel_before_dispatch_and_reject_unsupported_resume() {
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    for resume in ["", "mock-session"] {
        assert_eq!(
            request(
                &s,
                "POST",
                "/remote/v1/runs",
                &token,
                json!({"agent":"mock","prompt":"continue","resume":resume})
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let run = request(
        &s,
        "POST",
        "/remote/v1/runs",
        &token,
        json!({"agent":"mock","prompt":"cancel","idempotency_key":"cancelled"}),
    )
    .await
    .1;
    let path = format!("/remote/v1/runs/{}", run["id"].as_str().unwrap());
    assert_eq!(
        request(&s, "POST", &format!("{path}/cancel"), &token, Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    s.shutdown().await;
    assert_eq!(
        request(&s, "GET", &path, &token, Value::Null).await.1["state"],
        "cancelled"
    );
    drop(s);
    let s = remote(root.path());
    let retry = request(
        &s,
        "POST",
        "/remote/v1/runs",
        &token,
        json!({"agent":"mock","prompt":"cancel","idempotency_key":"cancelled"}),
    )
    .await
    .1;
    assert_eq!(retry["id"], run["id"]);
    assert_eq!(retry["state"], "cancelled");
    assert_eq!(
        request(&s, "DELETE", &path, &token, Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&s, "GET", &path, &token, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn persisted_events_page_without_gaps() {
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    drop(s);
    let db = rusqlite::Connection::open(root.path().join("home/state/remote.db")).unwrap();
    let events: Vec<_> = (1..=405)
        .map(|seq| json!({"seq":seq,"event":{"type":"turn_started"}}))
        .collect();
    let value = json!({"summary":{"id":"pages","state":"completed"},"events":events,"fingerprint":"f","idempotency_key":null,"bytes":0});
    db.execute(
        "INSERT INTO remote_runs(workspace,id,value) VALUES (?1,'pages',?2)",
        rusqlite::params![
            root.path().canonicalize().unwrap().to_string_lossy(),
            value.to_string()
        ],
    )
    .unwrap();
    drop(db);
    let s = remote(root.path());
    let mut collected = vec![];
    let mut cursor = 0;
    for expected_len in [200, 200, 5, 0] {
        let (status, page) = request(
            &s,
            "GET",
            &format!("/remote/v1/runs/pages/events?after={cursor}"),
            &token,
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let batch = page["events"].as_array().unwrap();
        assert_eq!(batch.len(), expected_len);
        collected.extend(batch.iter().cloned());
        cursor = page["next_cursor"].as_u64().unwrap();
    }
    assert_eq!(cursor, 405);
    assert_eq!(collected, events);
}

#[cfg(unix)]
#[tokio::test]
async fn native_manifests_resume_with_stdin_eof() {
    use std::os::unix::fs::PermissionsExt;
    for (name, manifest, output, expected) in [
        (
            "codex",
            include_str!("../../../manifeste/codex.toml"),
            r#"{"type":"thread.started","thread_id":"continued"}
{"type":"item.completed","item":{"type":"agent_message","text":"resumed"}}"#,
            vec![
                "exec",
                "resume",
                "--json",
                "--skip-git-repo-check",
                "--",
                "continued",
                "--help {model} {session_id}",
            ],
        ),
        (
            "opencode",
            include_str!("../../../manifeste/opencode.toml"),
            r#"{"type":"text","sessionID":"continued","part":{"text":"resumed"}}
{"type":"step_finish","sessionID":"continued"}"#,
            vec![
                "run",
                "--format",
                "json",
                "--session",
                "continued",
                "--",
                "--help {model} {session_id}",
            ],
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let command = root.path().join("fake-agent");
        std::fs::write(&command, format!("#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' \"$@\" > argv\ncat <<'EVENTS'\n{output}\nEVENTS\n")).unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
        let manifests = root.path().join("manifeste");
        std::fs::create_dir(&manifests).unwrap();
        std::fs::write(
            manifests.join(format!("{name}.toml")),
            manifest.replace(
                &format!("command = \"{name}\""),
                &format!("command = {:?}", command.to_str().unwrap()),
            ),
        )
        .unwrap();
        let s = remote(root.path());
        let token = pair(&s).await;
        let seeded = request(
            &s,
            "POST",
            "/remote/v1/runs",
            &token,
            json!({"agent":name,"prompt":"seed session"}),
        )
        .await
        .1;
        let seed_path = format!("/remote/v1/runs/{}", seeded["id"].as_str().unwrap());
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if request(&s, "GET", &seed_path, &token, Value::Null).await.1["state"] != "running"
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let (status, run) = request(
            &s,
            "POST",
            "/remote/v1/runs",
            &token,
            json!({"agent":name,"prompt":"--help {model} {session_id}","resume":"continued"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let path = format!("/remote/v1/runs/{}", run["id"].as_str().unwrap());
        let final_state = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let snapshot = request(&s, "GET", &path, &token, Value::Null).await.1;
                if snapshot["state"] != "running" {
                    break snapshot;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(final_state["state"], "completed");
        assert_eq!(final_state["session_id"], "continued");
        let args = std::fs::read_to_string(root.path().join("argv")).unwrap();
        assert_eq!(args.lines().collect::<Vec<_>>(), expected);
        let events = request(&s, "GET", &format!("{path}/events"), &token, Value::Null)
            .await
            .1;
        assert!(events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["event"]["text"] == "resumed"));
        s.shutdown().await;
        assert_eq!(
            request(
                &s,
                "POST",
                "/remote/v1/runs",
                &token,
                json!({"agent":name,"prompt":"late"})
            )
            .await
            .0,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}

#[tokio::test]
async fn failed_revocation_keeps_authority_consistent_with_disk() {
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    let id = s.devices()[0].device_id.clone();
    let db = rusqlite::Connection::open(root.path().join("home/state/remote.db")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_auth BEFORE UPDATE ON remote_state BEGIN SELECT RAISE(FAIL,'test failure'); END;").unwrap();
    assert!(s.revoke_device(&id).is_err());
    assert_eq!(s.devices().len(), 1);
    assert_eq!(
        request(&s, "DELETE", "/remote/v1/device", &token, Value::Null)
            .await
            .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        request(&s, "GET", "/remote/v1/info", &token, Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    db.execute_batch("DROP TRIGGER reject_auth").unwrap();
    assert_eq!(
        request(&s, "DELETE", "/remote/v1/device", &token, Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    drop(s);
    let s = remote(root.path());
    assert_eq!(
        request(&s, "GET", "/remote/v1/info", &token, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn failed_final_persistence_is_not_reported_as_completed() {
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    let db = rusqlite::Connection::open(root.path().join("home/state/remote.db")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_success BEFORE UPDATE ON remote_runs WHEN json_extract(NEW.value,'$.summary.state')='completed' BEGIN SELECT RAISE(FAIL,'final state unavailable'); END;").unwrap();
    let run = request(
        &s,
        "POST",
        "/remote/v1/runs",
        &token,
        json!({"agent":"mock","prompt":"storage failure"}),
    )
    .await
    .1;
    let path = format!("/remote/v1/runs/{}", run["id"].as_str().unwrap());
    let terminal = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let snapshot = request(&s, "GET", &path, &token, Value::Null).await.1;
            if snapshot["state"] != "running" {
                break snapshot;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(terminal["state"], "failed");
    assert_eq!(terminal["error_code"], "storage_failed");
    s.shutdown().await;
    drop(s);
    drop(db);
    let reopened = remote(root.path());
    let restored = request(&reopened, "GET", &path, &token, Value::Null)
        .await
        .1;
    assert_eq!(restored, terminal);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn oversized_unterminated_native_stdout_is_stopped_before_eof() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let command = root.path().join("oversized-codex");
    std::fs::write(&command, "#!/bin/sh\necho $$ > parent.pid\ndd if=/dev/zero bs=1048576 count=17 2>/dev/null | tr '\\000' x\nsleep 30\n").unwrap();
    std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(root.path().join("manifeste")).unwrap();
    std::fs::write(
        root.path().join("manifeste/codex.toml"),
        include_str!("../../../manifeste/codex.toml").replace(
            "command = \"codex\"",
            &format!("command = {:?}", command.to_str().unwrap()),
        ),
    )
    .unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    let run = request(
        &s,
        "POST",
        "/remote/v1/runs",
        &token,
        json!({"agent":"codex","prompt":"oversized output"}),
    )
    .await
    .1;
    let path = format!("/remote/v1/runs/{}", run["id"].as_str().unwrap());
    let terminal = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let snapshot = request(&s, "GET", &path, &token, Value::Null).await.1;
            if snapshot["state"] != "running" {
                break snapshot;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    // Always stop a baseline implementation before asserting, including when
    // its unbounded reader waits indefinitely for a newline that never comes.
    s.shutdown().await;
    let terminal = terminal.expect("the oversized reader must fail before stdout EOF");
    assert_eq!(terminal["state"], "failed");
    assert_eq!(terminal["error_code"], "output_limit_exceeded");
    assert_eq!(
        request(&s, "GET", &format!("{path}/events"), &token, Value::Null)
            .await
            .1["events"],
        json!([])
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn cancel_native_run_stops_child_and_descendant() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let command = root.path().join("fake-codex");
    std::fs::write(&command,"#!/bin/sh\necho $$ > parent.pid\nsleep 30 &\necho $! > descendant.pid\nprintf '%s\\n' '{\"type\":\"thread.started\",\"thread_id\":\"sleeping\"}'\nwait\n").unwrap();
    std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(root.path().join("manifeste")).unwrap();
    std::fs::write(
        root.path().join("manifeste/codex.toml"),
        include_str!("../../../manifeste/codex.toml").replace(
            "command = \"codex\"",
            &format!("command = {:?}", command.to_str().unwrap()),
        ),
    )
    .unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    let run = request(
        &s,
        "POST",
        "/remote/v1/runs",
        &token,
        json!({"agent":"codex","prompt":"wait"}),
    )
    .await
    .1;
    let path = format!("/remote/v1/runs/{}", run["id"].as_str().unwrap());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if request(&s, "GET", &path, &token, Value::Null).await.1["session_id"] == "sleeping" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let pids: Vec<_> = ["parent.pid", "descendant.pid"]
        .iter()
        .map(|file| {
            std::fs::read_to_string(root.path().join(file))
                .unwrap()
                .trim()
                .parse::<u32>()
                .unwrap()
        })
        .collect();
    assert_eq!(
        request(&s, "POST", &format!("{path}/cancel"), &token, Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    s.shutdown().await;
    assert_eq!(
        request(&s, "GET", &path, &token, Value::Null).await.1["state"],
        "cancelled"
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if pids.iter().all(
                |pid| match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
                    Err(_) => true,
                    Ok(stat) => stat
                        .rsplit_once(") ")
                        .is_some_and(|(_, fields)| fields.starts_with("Z ")),
                },
            ) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn folders_require_pairing_and_enforce_real_allowed_roots() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("project-a")).unwrap();
    std::fs::create_dir(root.path().join("project-b")).unwrap();
    std::fs::write(root.path().join("readme.txt"), "actual file").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
    let paths = OrchesterPaths::new(root.path().join("home"), root.path().join("project-a"));
    let s = RemoteControl::with_allowed_roots(
        ServerContext::new(Some(paths), ServerControl::new()),
        root.path().join("project-a"),
        vec![root.path().to_path_buf()],
    )
    .unwrap();
    for endpoint in ["/remote/v1/folders", "/remote/v1/projects"] {
        assert_eq!(
            request(&s, "GET", endpoint, "", Value::Null).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let token = pair(&s).await;
    let query = format!(
        "/remote/v1/folders?path={}&limit=1",
        root.path().to_str().unwrap()
    );
    let (status, page) = request(&s, "GET", &query, &token, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["entries"].as_array().unwrap().len(), 1);
    assert_eq!(page["entries"][0]["type"], "directory");
    assert_eq!(page["next_offset"], 1);
    assert!(page["parent_path"].is_null());
    let all = request(
        &s,
        "GET",
        &format!("/remote/v1/folders?path={}", root.path().to_str().unwrap()),
        &token,
        Value::Null,
    )
    .await
    .1;
    assert!(all["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "readme.txt" && e["type"] == "file"));
    assert!(all["entries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["name"] != "escape"));
    assert_eq!(
        request(
            &s,
            "GET",
            &format!(
                "/remote/v1/folders?path={}",
                outside.path().to_str().unwrap()
            ),
            &token,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &s,
            "POST",
            "/remote/v1/projects",
            &token,
            json!({"path":outside.path()})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    #[cfg(unix)]
    assert_eq!(
        request(
            &s,
            "GET",
            &format!(
                "/remote/v1/folders?path={}/escape",
                root.path().to_str().unwrap()
            ),
            &token,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &s,
            "POST",
            "/remote/v1/projects",
            &token,
            json!({"path":"relative"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        request(
            &s,
            "GET",
            "/remote/v1/folders?limit=201",
            &token,
            Value::Null
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn project_catalog_deduplicates_and_survives_restart_without_demo_agents() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let make = || {
        RemoteControl::with_allowed_roots(
            ServerContext::new(
                Some(OrchesterPaths::new(root.path().join("home"), &a)),
                ServerControl::new(),
            ),
            a.clone(),
            vec![root.path().to_path_buf()],
        )
        .unwrap()
    };
    let s = make();
    let token = pair(&s).await;
    let added = request(
        &s,
        "POST",
        "/remote/v1/projects",
        &token,
        json!({"path":b,"name":"第二个项目"}),
    )
    .await;
    assert_eq!(added.0, StatusCode::OK);
    assert_eq!(added.1["name"], "第二个项目");
    let repeat = request(&s, "POST", "/remote/v1/projects", &token, json!({"path":b}))
        .await
        .1;
    assert_eq!(repeat, added.1);
    let agents = request(&s, "GET", "/remote/v1/agents", &token, Value::Null)
        .await
        .1;
    assert!(agents["agents"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["name"] != "mock"));
    drop(s);
    let s = make();
    let projects = request(&s, "GET", "/remote/v1/projects", &token, Value::Null).await;
    assert_eq!(projects.0, StatusCode::OK);
    assert_eq!(projects.1["projects"].as_array().unwrap().len(), 2);
    assert!(projects.1["projects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == added.1["id"]));
}

#[cfg(unix)]
#[tokio::test]
async fn selected_projects_use_their_own_cli_directory_sessions_and_idempotency_scope() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    for path in [&a, &b] {
        std::fs::create_dir_all(path.join("manifeste")).unwrap();
        let script = path.join("agent.sh");
        std::fs::write(&script,"#!/bin/sh\nprintf '{\"type\":\"text\",\"session_id\":\"session-%s\",\"text\":\"%s\"}\\n' \"$(basename \"$PWD\")\" \"$PWD\"\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let manifest=format!("name=\"fixture\"\ncommand={}\nargs=[]\nsupports_resume=true\n[parse]\ndiscriminator=\"type\"\nsession_id=\"session_id\"\n[parse.map]\ntext={{event=\"message\",text=\"text\"}}\n",serde_json::to_string(&script).unwrap());
        std::fs::write(path.join("manifeste/fixture.toml"), manifest).unwrap();
    }
    let make = || {
        RemoteControl::with_allowed_roots(
            ServerContext::new(
                Some(OrchesterPaths::new(root.path().join("home"), &a)),
                ServerControl::new(),
            ),
            a.clone(),
            vec![root.path().to_path_buf()],
        )
        .unwrap()
    };
    let s = make();
    let token = pair(&s).await;
    let aid = request(&s, "GET", "/remote/v1/info", &token, Value::Null)
        .await
        .1["default_project_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let bid = request(&s, "POST", "/remote/v1/projects", &token, json!({"path":b}))
        .await
        .1["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut ids = Vec::new();
    for (project, path, session) in [(&aid, &a, "session-a"), (&bid, &b, "session-b")] {
        let payload = json!({"agent":"fixture","project_id":project,"prompt":"read cwd","idempotency_key":"same-key"});
        let (status, run) = request(&s, "POST", "/remote/v1/runs", &token, payload.clone()).await;
        assert_eq!(status, StatusCode::OK);
        let id = run["id"].as_str().unwrap().to_owned();
        ids.push(id.clone());
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if request(
                    &s,
                    "GET",
                    &format!("/remote/v1/runs/{id}"),
                    &token,
                    Value::Null,
                )
                .await
                .1["state"]
                    != "running"
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let finished = request(
            &s,
            "GET",
            &format!("/remote/v1/runs/{id}"),
            &token,
            Value::Null,
        )
        .await
        .1;
        assert_eq!(finished["state"], "completed");
        assert_eq!(finished["session_id"], session);
        assert_eq!(finished["workspace"], path.to_str().unwrap());
        let events = request(
            &s,
            "GET",
            &format!("/remote/v1/runs/{id}/events"),
            &token,
            Value::Null,
        )
        .await
        .1;
        assert!(events["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["event"]["text"] == path.to_str().unwrap()));
        assert_eq!(
            request(&s, "POST", "/remote/v1/runs", &token, payload)
                .await
                .1["id"],
            id
        );
    }
    assert_ne!(ids[0], ids[1]);
    assert_eq!(request(&s,"POST","/remote/v1/runs",&token,json!({"agent":"fixture","project_id":bid,"prompt":"cross-project resume","resume":"session-a"})).await.0,StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(request(&s,"POST","/remote/v1/runs",&token,json!({"agent":"fixture","project_id":bid,"prompt":"same-project resume","resume":"session-b"})).await.0,StatusCode::OK);
    s.shutdown().await;
    drop(s);
    let s = make();
    let list = request(
        &s,
        "GET",
        &format!("/remote/v1/runs?project_id={aid}"),
        &token,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(list["runs"].as_array().unwrap().len(), 1);
    assert_eq!(list["runs"][0]["id"], ids[0]);
    let list = request(
        &s,
        "GET",
        &format!("/remote/v1/runs?project_id={bid}"),
        &token,
        Value::Null,
    )
    .await
    .1;
    assert_eq!(list["runs"].as_array().unwrap().len(), 2);
}

#[cfg(unix)]
#[tokio::test]
async fn folders_skip_unaddressable_non_utf8_entries_instead_of_panicking() {
    use std::os::unix::ffi::OsStringExt;
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("normal.txt"), "normal").unwrap();
    std::fs::write(
        root.path().join(std::ffi::OsString::from_vec(vec![0xff])),
        "bytes",
    )
    .unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    let (status, page) = request(&s, "GET", "/remote/v1/folders", &token, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "normal.txt"));
}

#[cfg(unix)]
#[tokio::test]
async fn replacing_a_selected_project_with_another_project_symlink_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let s = RemoteControl::with_allowed_roots(
        ServerContext::new(
            Some(OrchesterPaths::new(root.path().join("home"), &a)),
            ServerControl::new(),
        ),
        a.clone(),
        vec![root.path().to_path_buf()],
    )
    .unwrap();
    let token = pair(&s).await;
    let id = request(&s, "GET", "/remote/v1/info", &token, Value::Null)
        .await
        .1["default_project_id"]
        .clone();
    std::fs::rename(&a, root.path().join("old-a")).unwrap();
    std::os::unix::fs::symlink(&b, &a).unwrap();
    assert_eq!(
        request(
            &s,
            "POST",
            "/remote/v1/runs",
            &token,
            json!({"agent":"codex","project_id":id,"prompt":"must not execute in b"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &s,
            "GET",
            &format!("/remote/v1/agents?project_id={}", id.as_str().unwrap()),
            &token,
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn selecting_a_legacy_workspace_imports_its_history_without_restart() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let make = || {
        RemoteControl::with_allowed_roots(
            ServerContext::new(
                Some(OrchesterPaths::new(root.path().join("home"), &a)),
                ServerControl::new(),
            ),
            a.clone(),
            vec![root.path().to_path_buf()],
        )
        .unwrap()
    };
    let s = make();
    let token = pair(&s).await;
    drop(s);
    let db = rusqlite::Connection::open(root.path().join("home/state/remote.db")).unwrap();
    let legacy = json!({"summary":{"id":"legacy-b","agent":"codex","prompt":"old b task","state":"completed","session_id":"old-b-session"},"events":[],"fingerprint":"old","idempotency_key":null});
    db.execute(
        "INSERT INTO remote_runs(workspace,id,value) VALUES (?1,?2,?3)",
        rusqlite::params![
            b.canonicalize().unwrap().to_string_lossy(),
            "legacy-b",
            legacy.to_string()
        ],
    )
    .unwrap();
    drop(db);
    let s = make();
    let added = request(&s, "POST", "/remote/v1/projects", &token, json!({"path":b}))
        .await
        .1;
    let (status, list) = request(
        &s,
        "GET",
        &format!(
            "/remote/v1/runs?project_id={}",
            added["id"].as_str().unwrap()
        ),
        &token,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["runs"].as_array().unwrap().len(), 1);
    assert_eq!(list["runs"][0]["id"], "legacy-b");
    assert_eq!(list["runs"][0]["session_id"], "old-b-session");
}

#[tokio::test]
async fn v1_idempotency_retries_keep_the_original_run_after_project_upgrade() {
    use sha2::{Digest, Sha256};
    let root = tempfile::tempdir().unwrap();
    let s = remote(root.path());
    let token = pair(&s).await;
    drop(s);
    let old =
        r#"{"agent":"mock","prompt":"legacy prompt","resume":null,"idempotency_key":"old-key"}"#;
    let fingerprint = format!("{:x}", Sha256::digest(old.as_bytes()));
    let legacy = json!({"summary":{"id":"legacy-v1","agent":"mock","prompt":"legacy prompt","state":"completed","session_id":"mock-session"},"events":[],"fingerprint":fingerprint,"idempotency_key":"old-key"});
    let db = rusqlite::Connection::open(root.path().join("home/state/remote.db")).unwrap();
    db.execute(
        "INSERT INTO remote_runs(workspace,id,value) VALUES (?1,?2,?3)",
        rusqlite::params![
            root.path().canonicalize().unwrap().to_string_lossy(),
            "legacy-v1",
            legacy.to_string()
        ],
    )
    .unwrap();
    drop(db);
    let s = remote(root.path());
    let (status, retry) = request(
        &s,
        "POST",
        "/remote/v1/runs",
        &token,
        serde_json::from_str(old).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry["id"], "legacy-v1");
}
