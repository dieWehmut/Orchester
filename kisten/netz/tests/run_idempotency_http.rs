//! Exercises the submit contract over real loopback HTTP and the returned WS URL.
//! The isolated home and empty configuration deliberately cannot call a model.

use std::{fs, net::SocketAddr, path::PathBuf, time::Duration};

use futures::{future::join_all, StreamExt};
use orchester_anwendung::OrchesterPaths;
use orchester_laufzeit::harness::config::{ConfigLoader, ConfigValue, USER_CONFIG};
use orchester_netz::{app_router, ServerContext, ServerControl};
use reqwest::{header::HeaderMap, Client, StatusCode};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Barrier,
    task::JoinHandle,
    time::timeout,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;

const DEADLINE: Duration = Duration::from_secs(5);

struct LoopbackFixture {
    root: PathBuf,
    address: SocketAddr,
    client: Client,
    shutdown: CancellationToken,
    server: JoinHandle<()>,
}

impl LoopbackFixture {
    async fn new() -> Self {
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce).expect("fixture nonce");
        let nonce: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir().join(format!("orchester-run-http-{nonce}"));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&root).expect("private fixture root");
        fs::create_dir(root.join("manifeste")).expect("fixture manifests");
        // Explicit empty user configuration prevents any ambient user model
        // selection or credentials from becoming part of this regression.
        let config_path = root.join("home").join(USER_CONFIG);
        ConfigLoader::for_user_path(&config_path)
            .edit_user_config(&[(
                vec!["model_providers".to_owned()],
                ConfigValue::Object(vec![]),
            )])
            .expect("private model configuration");
        fs::write(config_path, "{}").expect("empty model configuration");
        let paths = OrchesterPaths::new(root.join("home"), &root);
        let context = ServerContext::new(Some(paths), ServerControl::new());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("loopback bind");
        let address = listener.local_addr().expect("loopback address");
        let shutdown = CancellationToken::new();
        let stopped = shutdown.clone();
        let server = tokio::spawn(async move {
            axum::serve(listener, app_router(context))
                .with_graceful_shutdown(stopped.cancelled_owned())
                .await
                .expect("serve loopback router");
        });
        Self {
            root,
            address,
            client: Client::builder()
                .no_proxy()
                .timeout(DEADLINE)
                .build()
                .expect("HTTP client"),
            shutdown,
            server,
        }
    }

    fn url(&self, suffix: &str) -> String {
        format!("http://{}/api/v1{suffix}", self.address)
    }

    async fn finish(&mut self) {
        self.shutdown.cancel();
        timeout(DEADLINE, &mut self.server)
            .await
            .expect("server shutdown deadline")
            .expect("server task");
    }
}

impl Drop for LoopbackFixture {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.server.abort();
        // This is the freshly created, random, absolute root owned by this fixture.
        assert!(self.root.is_absolute() && self.root.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.root);
    }
}

async fn accepted(response: reqwest::Response) -> Value {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    response.json().await.expect("accepted JSON")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_http_retries_publish_one_initial_turn_and_a_working_replay_socket() {
    let mut fixture = LoopbackFixture::new().await;
    let submissions = 16;
    let barrier = Barrier::new(submissions);
    let responses = join_all((0..submissions).map(|_| async {
        barrier.wait().await;
        let started = accepted(
            fixture
                .client
                .post(fixture.url("/runs"))
                .header("idempotency-key", "one-concurrent-submission")
                .json(&json!({"prompt": "inspect the isolated HTTP fixture"}))
                .send()
                .await
                .expect("submit over HTTP"),
        )
        .await;
        // Every successful retry can immediately hydrate the first turn; this
        // catches a reservation published before its initial journal append.
        let run_id = started["run_id"].as_str().expect("run id");
        let snapshot = accepted(
            fixture
                .client
                .get(fixture.url(&format!("/runs/{run_id}")))
                .send()
                .await
                .expect("immediate snapshot"),
        )
        .await;
        assert_eq!(snapshot["events"][0]["sequence"], 1);
        assert_eq!(snapshot["events"][0]["kind"]["type"], "user_message");
        assert_eq!(
            snapshot["events"][0]["kind"]["text"],
            "inspect the isolated HTTP fixture"
        );
        assert_eq!(
            snapshot["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["kind"]["type"] == "user_message")
                .count(),
            1
        );
        started
    }))
    .await;
    assert!(responses.iter().all(|response| response == &responses[0]));
    let run_id = responses[0]["run_id"].as_str().unwrap();
    let events_url = responses[0]["events_url"].as_str().expect("events URL");
    assert_eq!(
        events_url,
        format!("ws://{}/api/v1/runs/{run_id}/events", fixture.address)
    );
    let failed = failed_snapshot(&fixture, run_id).await;

    let replay = accepted(
        fixture
            .client
            .post(fixture.url(&format!("/runs/{run_id}/replay")))
            .json(&json!({"after_sequence": 0}))
            .send()
            .await
            .expect("HTTP replay"),
    )
    .await;
    assert_eq!(replay["events"][0]["sequence"], 1);
    // Follow the actual server response verbatim: reconstructing a corrected
    // route here would conceal a broken /api/v1 prefix in the submit response.
    let (mut socket, handshake) = timeout(DEADLINE, connect_async(events_url))
        .await
        .expect("WS handshake deadline")
        .expect("returned URL upgrades");
    assert_eq!(handshake.status().as_u16(), 101);
    assert_eq!(replay["events"], failed["events"]);
    for expected in failed["events"].as_array().unwrap() {
        let frame = timeout(DEADLINE, socket.next())
            .await
            .expect("WS replay deadline")
            .expect("WS replay frame")
            .expect("WS replay message");
        let Message::Text(text) = frame else {
            panic!("expected replay JSON text")
        };
        let frame: Value = serde_json::from_str(&text).expect("WS replay JSON");
        assert_eq!(frame["type"], "event");
        assert_eq!(&frame["event"], expected);
    }
    socket.close(None).await.expect("close replay socket");

    let (mut resumed, _) = timeout(
        DEADLINE,
        connect_async(format!("{events_url}?after_sequence=1")),
    )
    .await
    .expect("WS cursor reconnect deadline")
    .expect("reconnect retained run");
    for expected in failed["events"].as_array().unwrap().iter().skip(1) {
        let frame = timeout(DEADLINE, resumed.next())
            .await
            .expect("WS cursor replay deadline")
            .expect("WS cursor replay frame")
            .expect("WS cursor replay message");
        let Message::Text(text) = frame else {
            panic!("expected cursor replay JSON text")
        };
        let frame: Value = serde_json::from_str(&text).expect("WS cursor replay JSON");
        assert_eq!(frame["type"], "event");
        assert_eq!(&frame["event"], expected);
    }
    resumed.close(None).await.expect("close resumed socket");

    accepted(
        fixture
            .client
            .post(fixture.url(&format!("/runs/{run_id}/cancel")))
            .send()
            .await
            .expect("stop isolated run"),
    )
    .await;
    let retry = accepted(
        fixture
            .client
            .post(fixture.url("/runs"))
            .header("idempotency-key", "one-concurrent-submission")
            .json(&json!({"prompt": "inspect the isolated HTTP fixture"}))
            .send()
            .await
            .expect("retry terminal run"),
    )
    .await;
    assert_eq!(retry, responses[0]);
    assert_eq!(failed_snapshot(&fixture, run_id).await, failed);
    fixture.finish().await;
}

#[tokio::test]
async fn a_lost_http_response_reuses_a_failed_run_instead_of_leaving_it_running() {
    let mut fixture = LoopbackFixture::new().await;
    let key = "retry-after-lost-response";
    let payload = json!({"prompt": "request with a deliberately discarded response"}).to_string();
    let mut transport = TcpStream::connect(fixture.address)
        .await
        .expect("raw HTTP connection");
    let request = format!(
        "POST /api/v1/runs HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nIdempotency-Key: {key}\r\nConnection: close\r\n\r\n{payload}",
        fixture.address, payload.len()
    );
    transport
        .write_all(request.as_bytes())
        .await
        .expect("send complete HTTP request");
    // Read only the status line, then lose the connection before decoding the
    // submit JSON. Acceptance is certain, but the caller never learns its id.
    let status = timeout(DEADLINE, async {
        let mut status = Vec::new();
        loop {
            let byte = transport.read_u8().await.expect("response status byte");
            status.push(byte);
            assert!(status.len() < 4096, "bounded status line");
            if byte == b'\n' {
                break;
            }
        }
        String::from_utf8(status).expect("ASCII HTTP status")
    })
    .await
    .expect("lost response status deadline");
    assert!(status.starts_with("HTTP/1.1 200 "));
    drop(transport);

    let retry = accepted(
        fixture
            .client
            .post(fixture.url("/runs"))
            .header("idempotency-key", key)
            .json(&serde_json::from_str::<Value>(&payload).unwrap())
            .send()
            .await
            .expect("retry after response loss"),
    )
    .await;
    let run_id = retry["run_id"].as_str().unwrap();
    let failed = failed_snapshot(&fixture, run_id).await;
    assert_eq!(
        failed["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["kind"]["type"] == "user_message")
            .count(),
        1
    );
    let repeated = accepted(
        fixture
            .client
            .post(fixture.url("/runs"))
            .header("idempotency-key", key)
            .json(&serde_json::from_str::<Value>(&payload).unwrap())
            .send()
            .await
            .expect("retry terminal result"),
    )
    .await;
    assert_eq!(repeated, retry);
    assert_eq!(failed_snapshot(&fixture, run_id).await, failed);
    fixture.finish().await;
}

async fn failed_snapshot(fixture: &LoopbackFixture, run_id: &str) -> Value {
    timeout(DEADLINE, async {
        loop {
            let snapshot = accepted(
                fixture
                    .client
                    .get(fixture.url(&format!("/runs/{run_id}")))
                    .send()
                    .await
                    .expect("poll isolated run"),
            )
            .await;
            if snapshot["state"] == "failed" {
                let events = snapshot["events"].as_array().expect("failed event journal");
                assert!(events.iter().any(|event| event["kind"]["type"] == "error"));
                assert_eq!(events.last().unwrap()["kind"]["type"], "run_stopped");
                assert_eq!(events.last().unwrap()["kind"]["reason"], "failed");
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| event["kind"]["type"] == "run_stopped")
                        .count(),
                    1
                );
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("unconfigured runtime must publish a failed terminal run")
}

#[tokio::test]
async fn http_conflicts_and_validation_errors_do_not_echo_submission_secrets() {
    let mut fixture = LoopbackFixture::new().await;
    let key = "private-idempotency-key-not-for-error-bodies";
    let original = "private original prompt marker";
    let changed = "private conflicting prompt marker";
    let initial = accepted(
        fixture
            .client
            .post(fixture.url("/runs"))
            .header("idempotency-key", key)
            .json(&json!({"prompt": original}))
            .send()
            .await
            .expect("original submit"),
    )
    .await;
    let conflict = fixture
        .client
        .post(fixture.url("/runs"))
        .header("idempotency-key", key)
        .json(&json!({"prompt": changed}))
        .send()
        .await
        .expect("conflicting submit");
    assert_redacted_error(
        conflict,
        StatusCode::CONFLICT,
        "conflict",
        &[key, original, changed],
    )
    .await;

    let invalid_key = "private invalid key marker";
    let invalid = fixture
        .client
        .post(fixture.url("/runs"))
        .header("idempotency-key", invalid_key)
        .json(&json!({"prompt": changed}))
        .send()
        .await
        .expect("invalid key submit");
    assert_redacted_error(
        invalid,
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
        &[invalid_key, changed],
    )
    .await;

    let mut duplicate_headers = HeaderMap::new();
    duplicate_headers.append("idempotency-key", key.parse().unwrap());
    duplicate_headers.append("idempotency-key", key.parse().unwrap());
    let duplicate = fixture
        .client
        .post(fixture.url("/runs"))
        .headers(duplicate_headers)
        .json(&json!({"prompt": original}))
        .send()
        .await
        .expect("duplicate header submit");
    assert_redacted_error(
        duplicate,
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
        &[key, original],
    )
    .await;

    let unchanged = accepted(
        fixture
            .client
            .post(fixture.url("/runs"))
            .header("idempotency-key", key)
            .json(&json!({"prompt": original}))
            .send()
            .await
            .expect("original still reusable"),
    )
    .await;
    assert_eq!(unchanged, initial);
    let run_id = initial["run_id"].as_str().unwrap();
    accepted(
        fixture
            .client
            .post(fixture.url(&format!("/runs/{run_id}/cancel")))
            .send()
            .await
            .expect("stop isolated run"),
    )
    .await;
    fixture.finish().await;
}

async fn assert_redacted_error(
    response: reqwest::Response,
    status: StatusCode,
    code: &str,
    secrets: &[&str],
) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let text = response.text().await.expect("error body");
    for secret in secrets {
        assert!(
            !text.contains(secret),
            "error must not echo submitted input"
        );
    }
    let error: Value = serde_json::from_str(&text).expect("error JSON");
    assert_eq!(error["code"], code);
    assert_eq!(error["retryable"], false);
}
