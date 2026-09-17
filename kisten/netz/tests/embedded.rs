#![cfg(feature = "static-files")]

use std::{path::PathBuf, time::Duration};

use futures::StreamExt;
use orchester_anwendung::OrchesterPaths;
use orchester_netz::EmbeddedServer;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::client::IntoClientRequest};

struct Assets(PathBuf);

impl Assets {
    fn new() -> Self {
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let root = std::env::temp_dir().join(format!("orchester-embedded-{:x}", u128::from_ne_bytes(random)));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.html"), "<!doctype html><title>Orchester</title>").unwrap();
        Self(root)
    }
}

impl Drop for Assets {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn start(assets: &Assets) -> EmbeddedServer {
    EmbeddedServer::start(
        OrchesterPaths::new(assets.0.join("home"), assets.0.join("workspace")),
        assets.0.clone(),
    ).await.expect("start embedded server")
}

async fn authenticate(client: &Client, server: &EmbeddedServer) -> (String, String) {
    let launch = server.launch_url();
    let token = launch.split("#fragment_token=").nth(1).expect("launch token");
    let response = client.post(format!("{}/api/v1/auth/fragment", server.origin()))
        .header("origin", server.origin())
        .json(&json!({"schema_version": 1, "fragment_token": token}))
        .send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
    let session: Value = response.json().await.unwrap();
    (cookie, session["csrf_token"].as_str().unwrap().to_owned())
}

#[tokio::test]
async fn embedded_runtime_serves_assets_authenticated_api_and_websocket_then_releases_port() {
    let assets = Assets::new();
    let mut server = start(&assets).await;
    let address = server.address();
    assert_eq!(address.ip().to_string(), "127.0.0.1");
    assert_ne!(address.port(), 0);
    let client = Client::new();
    let response = client.get(server.origin()).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let csp = response.headers()["content-security-policy"].to_str().unwrap();
    assert!(csp.contains(&format!("ws://{address}")));
    assert!(!csp.contains("127.0.0.1:*"));
    assert!(response.text().await.unwrap().contains("Orchester"));
    let (cookie, _) = authenticate(&client, &server).await;
    for path in ["bootstrap", "agents", "agents/status", "models", "sessions", "session"] {
        let response = client.get(format!("{}/api/v1/{path}", server.origin()))
            .header("cookie", &cookie).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
    let mut request = format!("ws://{address}/api/v1/agents/status/ws").into_client_request().unwrap();
    request.headers_mut().insert("origin", server.origin().parse().unwrap());
    request.headers_mut().insert("cookie", cookie.parse().unwrap());
    let (mut socket, _) = connect_async(request).await.expect("authenticated websocket");
    let frame = tokio::time::timeout(Duration::from_secs(5), socket.next()).await.unwrap().unwrap().unwrap();
    let frame: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
    assert_eq!(frame["type"], "snapshot");
    server.shutdown().await.unwrap();
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    assert!(tokio::net::TcpListener::bind(address).await.is_ok());
}

#[tokio::test]
async fn embedded_runtime_rejects_unauthenticated_cross_origin_and_replayed_requests() {
    let assets = Assets::new();
    let mut server = start(&assets).await;
    let client = Client::new();
    for path in ["agents", "agents/status", "models", "sessions", "session"] {
        assert_eq!(client.get(format!("{}/api/v1/{path}", server.origin())).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    }
    assert_eq!(client.get(server.origin()).header("host", "attacker.example").send().await.unwrap().status(), StatusCode::FORBIDDEN);
    let (cookie, csrf) = authenticate(&client, &server).await;
    let fragment = server.launch_url().split("#fragment_token=").nth(1).unwrap().to_owned();
    assert_eq!(client.post(format!("{}/api/v1/auth/fragment", server.origin()))
        .header("origin", server.origin()).json(&json!({"schema_version": 1, "fragment_token": fragment}))
        .send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    for origin in ["https://attacker.example", "null", "http://127.0.0.1:1"] {
        assert_eq!(client.get(format!("{}/api/v1/agents", server.origin())).header("cookie", &cookie)
            .header("origin", origin).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    }
    let revoke = format!("{}/api/v1/session/revoke", server.origin());
    assert_eq!(client.post(&revoke).header("origin", server.origin()).header("cookie", &cookie).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    assert_eq!(client.post(&revoke).header("cookie", &cookie).header("x-csrf-token", &csrf).send().await.unwrap().status(), StatusCode::FORBIDDEN);
    let ws_url = format!("ws://{}/api/v1/agents/status/ws", server.address());
    assert!(connect_async(&ws_url).await.is_err());
    let mut request = ws_url.into_client_request().unwrap();
    request.headers_mut().insert("cookie", cookie.parse().unwrap());
    request.headers_mut().insert("origin", "https://attacker.example".parse().unwrap());
    assert!(connect_async(request).await.is_err());
    assert_eq!(client.post(&revoke).header("origin", server.origin()).header("cookie", &cookie)
        .header("x-csrf-token", csrf).send().await.unwrap().status(), StatusCode::NO_CONTENT);
    assert_eq!(client.get(format!("{}/api/v1/agents", server.origin())).header("cookie", &cookie).send().await.unwrap().status(), StatusCode::UNAUTHORIZED);
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn missing_bundle_assets_fail_startup() {
    let assets = Assets::new();
    assert!(EmbeddedServer::start(OrchesterPaths::new(&assets.0, &assets.0), assets.0.join("missing")).await.is_err());
}
