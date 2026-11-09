//! Only isolated shell fixtures are launched here; no Agent executables.
use axum::http::StatusCode;
use futures::{SinkExt, StreamExt};
use orchester_anwendung::OrchesterPaths;
use orchester_netz::{RemoteControl, ServerContext, ServerControl};
use serde_json::{json, Value};
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
struct Host {
    remote: RemoteControl,
    root: tempfile::TempDir,
    base: String,
    token: String,
    server: tokio::task::JoinHandle<()>,
}
impl Host {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = OrchesterPaths::new(root.path().join("home"), root.path());
        let remote = RemoteControl::new(
            ServerContext::new(Some(paths), ServerControl::new())
                .with_registry(orchester_verzeichnis::Registry::new()),
            root.path().to_path_buf(),
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let router = remote.router();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let token = reqwest::Client::new()
            .post(format!("{base}/remote/v1/pair"))
            .json(&json!({"code":remote.pairing_code(),"device_name":"terminal-test"}))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["token"]
            .as_str()
            .unwrap()
            .to_owned();
        Self {
            remote,
            root,
            base,
            token,
            server,
        }
    }
    async fn request(&self, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
        self.request_token(method, path, body, &self.token).await
    }
    async fn request_token(
        &self,
        method: &str,
        path: &str,
        body: Value,
        token: &str,
    ) -> (StatusCode, Value) {
        let response = reqwest::Client::new()
            .request(method.parse().unwrap(), format!("{}{path}", self.base))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body = response.json().await.unwrap_or(Value::Null);
        (status, body)
    }
    async fn create(&self) -> String {
        let (status, terminal) = self
            .request("POST", "/remote/v1/terminals", json!({"cols":80,"rows":24}))
            .await;
        assert_eq!(status, StatusCode::OK, "{terminal}");
        assert_eq!(terminal["state"], "running");
        assert_eq!(
            std::path::Path::new(terminal["workspace"].as_str().unwrap()),
            self.root.path().canonicalize().unwrap()
        );
        terminal["id"].as_str().unwrap().to_owned()
    }
    async fn ticket(&self, id: &str, after: u64) -> String {
        let (status, ticket) = self
            .request(
                "POST",
                &format!("/remote/v1/terminals/{id}/ticket"),
                json!({"after":after}),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        format!(
            "{}{}",
            self.base.replacen("http:", "ws:", 1),
            ticket["ws_path"].as_str().unwrap()
        )
    }
    async fn socket(&self, id: &str, after: u64) -> Socket {
        let (mut socket, _) = connect_async(self.ticket(id, after).await).await.unwrap();
        assert_eq!(recv(&mut socket).await["type"], "ready");
        socket
    }
    async fn stopped(&self, id: &str) -> Value {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let (_, terminal) = self
                    .request("GET", &format!("/remote/v1/terminals/{id}"), Value::Null)
                    .await;
                if terminal["state"] != "running" {
                    return terminal;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap()
    }
    async fn shutdown(self) {
        self.remote.shutdown().await;
        self.server.abort();
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn recv(socket: &mut Socket) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Message::Text(text) => {
                    let event: Value = serde_json::from_str(&text).unwrap();
                    return event;
                }
                Message::Ping(data) => socket.send(Message::Pong(data)).await.unwrap(),
                message => panic!("unexpected socket message {message:?}"),
            }
        }
    })
    .await
    .unwrap()
}
async fn input(socket: &mut Socket, data: &str) {
    socket
        .send(Message::Text(
            json!({"type":"input","data":data}).to_string().into(),
        ))
        .await
        .unwrap();
}
async fn output_until(socket: &mut Socket, expected: &str) -> (String, u64) {
    let mut output = String::new();
    let mut after = 0;
    loop {
        let event = recv(socket).await;
        if event["type"] == "output" {
            after = event["seq"].as_u64().unwrap();
            output.push_str(event["data"].as_str().unwrap());
        }
        assert_ne!(event["type"], "error", "{event}");
        assert_ne!(event["type"], "exit", "{output}");
        if output.contains(expected) {
            return (output, after);
        }
    }
}
async fn error(socket: &mut Socket) -> Value {
    loop {
        let event = recv(socket).await;
        if event["type"] == "error" {
            return event;
        }
        assert_ne!(event["type"], "exit", "{event}");
    }
}

#[tokio::test]
async fn shell_input_unicode_resize_replay_and_exit_are_real() {
    let host = Host::new().await;
    let id = host.create().await;
    let mut socket = host.socket(&id, 0).await;
    #[cfg(unix)]
    input(&mut socket, "stty -echo\rprintf '\\160\\164\\171-ready\\n'; pwd; printf '\\344\\275\\240\\345\\245\\275-terminal\\n'\r").await;
    #[cfg(windows)]
    input(
        &mut socket,
        "chcp 65001\recho pty-ready\recho 你好-terminal\rcd\r",
    )
    .await;
    let (text, _) = output_until(&mut socket, "你好-terminal").await;
    assert!(text.contains("pty-ready"), "{text}");
    assert!(!text.contains('\u{fffd}'), "{text}");
    // Validate the shell's actual cwd, including Windows canonical path handling.
    #[cfg(unix)]
    input(&mut socket, "pwd\r").await;
    #[cfg(unix)]
    output_until(&mut socket, host.root.path().to_str().unwrap()).await;
    #[cfg(windows)]
    {
        input(&mut socket, "cd\r").await;
        output_until(&mut socket, host.root.path().to_str().unwrap()).await;
    }
    #[cfg(unix)]
    input(
        &mut socket,
        "printf '\\146\\151\\170\\164\\165\\162\\145-prompt\\n'; read -r answer; printf 'answer=%s\\n' \"$answer\"\r",
    )
    .await;
    #[cfg(windows)]
    input(&mut socket, "set /p answer=fixture-prompt\r").await;
    output_until(&mut socket, "fixture-prompt").await;
    input(&mut socket, "fixture-input\r").await;
    #[cfg(windows)]
    input(&mut socket, "echo answer=%answer%\r").await;
    output_until(&mut socket, "answer=fixture-input").await;
    socket
        .send(Message::Text(
            json!({"type":"resize","cols":103,"rows":37})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let terminal = host
                .request("GET", &format!("/remote/v1/terminals/{id}"), Value::Null)
                .await
                .1;
            if terminal["cols"] == 103 && terminal["rows"] == 37 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    #[cfg(unix)]
    {
        input(&mut socket, "stty size\r").await;
        output_until(&mut socket, "37 103").await;
    }
    #[cfg(unix)]
    input(&mut socket, "echo replay-marker\r").await;
    #[cfg(windows)]
    input(&mut socket, "echo re^play-marker\r").await;
    let (_, cursor) = output_until(&mut socket, "replay-marker").await;
    socket.close(None).await.unwrap();
    drop(socket);
    tokio::time::sleep(Duration::from_millis(80)).await;
    let mut socket = host.socket(&id, 0).await;
    let (_, replayed) = output_until(&mut socket, "replay-marker").await;
    assert!(replayed >= cursor);
    input(&mut socket, "exit 7\r").await;
    loop {
        let event = recv(&mut socket).await;
        if event["type"] == "exit" {
            assert_eq!(event["terminal"]["state"], "exited");
            assert_eq!(event["terminal"]["exit_code"], 7);
            break;
        }
    }
    assert_eq!(host.stopped(&id).await["state"], "exited");
    host.shutdown().await;
}

#[tokio::test]
async fn authorization_ticket_binding_limits_and_shutdown() {
    let host = Host::new().await;
    assert_eq!(
        host.request_token("POST", "/remote/v1/terminals", json!({}), "wrong")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    for body in [
        json!({"cwd":"/tmp"}),
        json!({"command":"echo wrong"}),
        json!({"cols":1}),
        json!({"rows":301}),
    ] {
        assert_eq!(
            host.request("POST", "/remote/v1/terminals", body).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        host.request(
            "POST",
            "/remote/v1/terminals",
            json!({"project_id":"missing"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let id = host.create().await;
    let wrong_id = host.create().await;
    let code = host.remote.rotate_pairing_code().unwrap();
    let (_, second_device) = host
        .request(
            "POST",
            "/remote/v1/pair",
            json!({"code":code,"device_name":"other-device"}),
        )
        .await;
    let second_token = second_device["token"].as_str().unwrap();
    assert_eq!(
        host.request_token("GET", "/remote/v1/terminals", Value::Null, second_token)
            .await
            .1["terminals"],
        json!([])
    );
    assert_eq!(
        host.request_token(
            "GET",
            &format!("/remote/v1/terminals/{id}"),
            Value::Null,
            second_token
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        host.request_token(
            "DELETE",
            &format!("/remote/v1/terminals/{id}"),
            Value::Null,
            second_token
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        host.request_token(
            "POST",
            &format!("/remote/v1/terminals/{id}/ticket"),
            json!({}),
            second_token
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        host.request(
            "POST",
            &format!("/remote/v1/terminals/{id}/ticket"),
            json!({"after":u64::MAX})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let url = host.ticket(&id, 0).await;
    let wrong = url.replace(&id, &wrong_id);
    assert!(connect_async(wrong).await.is_err());
    assert!(
        connect_async(url).await.is_err(),
        "wrong-path attempt consumes single-use ticket"
    );
    let url = host.ticket(&id, 0).await;
    let (mut socket, _) = connect_async(&url).await.unwrap();
    assert_eq!(recv(&mut socket).await["type"], "ready");
    assert!(
        connect_async(url).await.is_err(),
        "ticket cannot be replayed"
    );
    assert!(
        connect_async(host.ticket(&id, 0).await).await.is_err(),
        "only one active socket"
    );
    for _ in 0..2 {
        host.create().await;
    }
    assert_eq!(
        host.request("POST", "/remote/v1/terminals", json!({}))
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let ticket_before_revoke = host.ticket(&wrong_id, 0).await;
    assert_eq!(
        host.request("DELETE", "/remote/v1/device", Value::Null)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert!(connect_async(ticket_before_revoke).await.is_err());
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        host.request("GET", "/remote/v1/terminals", Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    host.shutdown().await;
}

#[tokio::test]
async fn project_cwd_revalidation_resume_cursor_and_invalid_messages() {
    let host = Host::new().await;
    let project_dir = host.root.path().join("project 你好");
    std::fs::create_dir(&project_dir).unwrap();
    let (status, project) = host
        .request("POST", "/remote/v1/projects", json!({"path":project_dir}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let project_id = project["id"].as_str().unwrap();
    let (status, terminal) = host
        .request(
            "POST",
            "/remote/v1/terminals",
            json!({"project_id":project_id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(terminal["project_id"], project_id);
    assert_eq!(terminal["workspace"], project["path"]);
    let id = terminal["id"].as_str().unwrap();
    host.create().await;
    let (_, list) = host
        .request(
            "GET",
            &format!("/remote/v1/terminals?project_id={project_id}"),
            Value::Null,
        )
        .await;
    assert_eq!(list["terminals"].as_array().unwrap().len(), 1);
    assert_eq!(list["terminals"][0]["id"], id);

    let mut socket = host.socket(id, 0).await;
    #[cfg(unix)]
    input(
        &mut socket,
        "stty -echo\rprintf '\\160\\162\\157\\152\\145\\143\\164-cwd-ready\\n'; pwd\r",
    )
    .await;
    #[cfg(windows)]
    input(&mut socket, "chcp 65001\recho project-cwd-re^ady\rcd\r").await;
    output_until(&mut socket, "project-cwd-ready").await;
    #[cfg(unix)]
    input(&mut socket, "pwd\r").await;
    #[cfg(windows)]
    input(&mut socket, "cd\r").await;
    let (_, cursor) = output_until(&mut socket, project_dir.to_str().unwrap()).await;

    for message in [
        json!({"type":"input","data":"x","unexpected":true}),
        json!({"type":"resize","cols":1,"rows":24}),
        json!({"type":"resize","cols":80,"rows":301}),
        json!({"type":"input","data":"x".repeat(8193)}),
        json!({"type":"unknown"}),
    ] {
        socket
            .send(Message::Text(message.to_string().into()))
            .await
            .unwrap();
        assert_eq!(error(&mut socket).await["code"], "invalid_message");
    }
    socket.send(Message::Binary(vec![0].into())).await.unwrap();
    assert_eq!(error(&mut socket).await["code"], "invalid_message");
    socket.close(None).await.unwrap();
    drop(socket);
    tokio::time::sleep(Duration::from_millis(80)).await;
    let mut socket = host.socket(id, cursor).await;
    #[cfg(unix)]
    input(&mut socket, "echo resume-cursor-marker\r").await;
    #[cfg(windows)]
    input(&mut socket, "echo resume-cursor-mar^ker\r").await;
    let (output, next_cursor) = output_until(&mut socket, "resume-cursor-marker").await;
    assert!(next_cursor > cursor);
    assert!(!output.contains("project-cwd-ready"));

    host.request("DELETE", &format!("/remote/v1/terminals/{id}"), Value::Null)
        .await;
    assert_eq!(host.stopped(id).await["state"], "closed");
    std::fs::rename(&project_dir, host.root.path().join("moved-project")).unwrap();
    assert_eq!(
        host.request(
            "POST",
            "/remote/v1/terminals",
            json!({"project_id":project_id})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), &project_dir).unwrap();
        assert_eq!(
            host.request(
                "POST",
                "/remote/v1/terminals",
                json!({"project_id":project_id})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    host.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn replay_overflow_reports_gap_then_replays_retained_output() {
    let host = Host::new().await;
    let id = host.create().await;
    let mut socket = host.socket(&id, 0).await;
    input(&mut socket, "stty -echo\rprintf '%0300000d' 0; printf '\\n\\157\\166\\145\\162\\146\\154\\157\\167-marker\\n'\r").await;
    let (original, cursor) = output_until(&mut socket, "overflow-marker").await;
    assert!(original.len() > 256 * 1024);
    socket.close(None).await.unwrap();
    drop(socket);
    tokio::time::sleep(Duration::from_millis(80)).await;
    let mut socket = host.socket(&id, 0).await;
    let gap = recv(&mut socket).await;
    assert_eq!(gap["type"], "gap");
    assert!(gap["first_seq"].as_u64().unwrap() > 1);
    let (replayed, replay_cursor) = output_until(&mut socket, "overflow-marker").await;
    assert!(replayed.len() <= 256 * 1024);
    assert!(replay_cursor >= cursor);
    host.shutdown().await;
}

#[tokio::test]
async fn closing_terminal_kills_background_descendant_jobs() {
    let host = Host::new().await;
    let id = host.create().await;
    let mut socket = host.socket(&id, 0).await;
    #[cfg(unix)]
    {
        input(&mut socket, "stty -echo\r").await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        input(&mut socket, "sleep 30 & printf 'descendant=%s\\n' $!\r").await;
    }
    #[cfg(windows)]
    input(&mut socket, "powershell.exe -NoProfile -NonInteractive -Command \"$p=Start-Process cmd.exe -ArgumentList '/D','/C','ping -n 30 127.0.0.1 >nul' -WindowStyle Hidden -PassThru; Write-Output ('descendant=' + $p.Id)\"\r").await;
    let mut output = String::new();
    let pid: u32 = loop {
        let event = recv(&mut socket).await;
        if let Some(data) = event["data"].as_str() {
            output.push_str(data);
        }
        if let Some(pid) = output.split("descendant=").skip(1).find_map(|part| {
            part.trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .ok()
        }) {
            break pid;
        }
    };
    host.request("DELETE", &format!("/remote/v1/terminals/{id}"), Value::Null)
        .await;
    assert_eq!(host.stopped(&id).await["state"], "closed");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let system = sysinfo::System::new_all();
            let alive = system
                .process(sysinfo::Pid::from_u32(pid))
                .is_some_and(|p| p.status() != sysinfo::ProcessStatus::Zombie);
            if !alive {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    host.shutdown().await;
}
