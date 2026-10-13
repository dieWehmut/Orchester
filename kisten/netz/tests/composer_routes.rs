use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

async fn request(
    c: &ServerContext,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app_router(c.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(format!("/api/v1{path}"))
                .header("content-type", "application/json")
                .body(
                    body.map(|v| Body::from(v.to_string()))
                        .unwrap_or_else(Body::empty),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let bytes = to_bytes(response.into_body(), 3 * 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
fn fixture() -> (std::path::PathBuf, ServerContext) {
    let root = std::env::temp_dir().join(format!(
        "orchester-composer-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let context = ServerContext::new(
        Some(OrchesterPaths::new(root.join("home"), &root)),
        ServerControl::new(),
    );
    (root, context)
}
#[tokio::test]
async fn context_and_servers_survive_restart_and_do_not_cross_workspaces() {
    let (root, c) = fixture();
    let initial = request(&c, "GET", "/composer/context", None).await.1;
    assert_eq!(initial, json!({"revision":0,"goal":null,"plan_mode":false}));
    let input =
        json!({"revision":0,"goal":{"text":"Ship the report","status":"active"},"plan_mode":true});
    assert_eq!(
        request(&c, "PUT", "/composer/context", Some(input.clone()))
            .await
            .1["revision"],
        1
    );
    assert_eq!(
        request(&c, "PUT", "/composer/context", Some(input)).await.0,
        StatusCode::CONFLICT
    );
    let restored = ServerContext::new(c.paths().cloned(), ServerControl::new());
    assert_eq!(
        request(&restored, "GET", "/composer/context", None).await.1["goal"]["text"],
        "Ship the report"
    );
    let input = json!({"name":"Local","transport":"http","url":"http://127.0.0.1:9999/mcp","token_env":"MCP_TEST_TOKEN"});
    let (status, server) = request(&c, "POST", "/mcp/servers", Some(input)).await;
    assert_eq!(status, StatusCode::OK);
    let id = server["id"].as_str().unwrap();
    assert_eq!(
        request(&restored, "GET", "/mcp/servers", None).await.1[0]["token_env"],
        "MCP_TEST_TOKEN"
    );
    let other = ServerContext::new(
        Some(c.paths().unwrap().with_workspace(root.join("other"))),
        ServerControl::new(),
    );
    assert_eq!(
        request(&other, "GET", "/composer/context", None).await.1,
        initial
    );
    assert_eq!(
        request(&other, "GET", "/mcp/servers", None).await.1,
        json!([])
    );
    assert_eq!(
        request(&other, "DELETE", &format!("/mcp/servers/{id}"), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&c, "DELETE", &format!("/mcp/servers/{id}"), None)
            .await
            .0,
        StatusCode::OK
    );
    drop((c, restored, other));
    std::fs::remove_dir_all(root).unwrap();
}

async fn mock(
    State(log): State<Arc<Mutex<Vec<Value>>>>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> Response {
    log.lock().unwrap().push(input.clone());
    assert!(headers["accept"]
        .to_str()
        .unwrap()
        .contains("text/event-stream"));
    if input["method"] != "initialize" {
        assert_eq!(headers["mcp-session-id"], "test-session");
        assert_eq!(headers["mcp-protocol-version"], "2025-06-18");
    }
    let result=match input["method"].as_str().unwrap() {
        "initialize"=>return ([("Mcp-Session-Id","test-session")],Json(json!({"jsonrpc":"2.0","id":input["id"],"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"Fixture","version":"1"}}}))).into_response(),
        "notifications/initialized"=>return StatusCode::ACCEPTED.into_response(),
        "tools/list" if input["params"].get("cursor").is_none()=>json!({"tools":[{"name":"echo","inputSchema":{"type":"object"}}],"nextCursor":"p2"}),
        "tools/list"=>json!({"tools":[{"name":"second","inputSchema":{"type":"object"}}]}),
        "resources/list"=>json!({"resources":[{"name":"Readme","uri":"test://readme"}]}),
        "prompts/list"=>json!({"prompts":[{"name":"Review"}]}),
        "tools/call"=>return ([("content-type","text/event-stream")],format!("event: message\r\ndata: {}\r\n\r\n",json!({"jsonrpc":"2.0","id":input["id"],"result":{"content":[{"type":"text","text":input["params"]["arguments"]["text"]}],"isError":false}}))).into_response(),
        "resources/read"=>json!({"contents":[{"uri":"test://readme","text":"hello"}]}),
        "prompts/get"=>json!({"messages":[{"role":"user","content":{"type":"text","text":"Review this"}}]}),
        _=>panic!("unexpected MCP method"),
    };
    Json(json!({"jsonrpc":"2.0","id":input["id"],"result":result})).into_response()
}
#[tokio::test]
async fn http_mcp_negotiates_sessions_pagination_and_streamed_results() {
    let (root, c) = fixture();
    let log = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route(
            "/mcp",
            post(mock).delete(|| async { StatusCode::NO_CONTENT }),
        )
        .with_state(log.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let (_, definition) = request(
        &c,
        "POST",
        "/mcp/servers",
        Some(json!({"name":"Fixture","transport":"http","url":format!("http://{address}/mcp")})),
    )
    .await;
    let id = definition["id"].as_str().unwrap();
    let (status, catalog) = request(&c, "POST", &format!("/mcp/servers/{id}/catalog"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["tools"].as_array().unwrap().len(), 2);
    assert_eq!(catalog["resources"][0]["uri"], "test://readme");
    let (status, result) = request(
        &c,
        "POST",
        &format!("/mcp/servers/{id}/invoke"),
        Some(
            json!({"method":"tools/call","params":{"name":"echo","arguments":{"text":"图像测试"}}}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["content"][0]["text"], "图像测试");
    assert_eq!(
        log.lock()
            .unwrap()
            .iter()
            .filter(|v| v["method"] == "tools/call")
            .count(),
        1
    );
    assert_eq!(
        request(
            &c,
            "POST",
            &format!("/mcp/servers/{id}/invoke"),
            Some(json!({"method":"arbitrary/execute","params":{}}))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    for (method, params, key) in [
        ("resources/read", json!({"uri":"test://readme"}), "contents"),
        ("prompts/get", json!({"name":"Review"}), "messages"),
    ] {
        let (_, value) = request(
            &c,
            "POST",
            &format!("/mcp/servers/{id}/invoke"),
            Some(json!({"method":method,"params":params})),
        )
        .await;
        assert!(value[key].is_array());
    }
    server.abort();
    drop(c);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires Node.js; included in desktop acceptance"]
async fn stdio_mcp_handshake_and_call_use_a_direct_background_process() {
    let (root, c) = fixture();
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp-server.mjs");
    let (_,definition)=request(&c,"POST","/mcp/servers",Some(json!({"name":"Local fixture","transport":"stdio","command":"node","args":[script.to_string_lossy()]}))).await;
    let id = definition["id"].as_str().unwrap();
    let (status, catalog) = request(&c, "POST", &format!("/mcp/servers/{id}/catalog"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["tools"][0]["name"], "echo");
    let (status, result) = request(
        &c,
        "POST",
        &format!("/mcp/servers/{id}/invoke"),
        Some(json!({"method":"tools/call","params":{"name":"echo","arguments":{"text":"works"}}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["content"][0]["text"], "works");
    drop(c);
    std::fs::remove_dir_all(root).unwrap();
}
