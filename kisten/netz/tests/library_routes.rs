use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};
use tower::ServiceExt;
#[tokio::test]
async fn library_routes_enforce_csrf_and_return_canonical_workspace() {
    let root = std::env::temp_dir().join(format!(
        "library-http-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("workspace")).unwrap();
    let router = app_router(ServerContext::new(
        Some(OrchesterPaths::new(
            root.join("home"),
            root.join("workspace"),
        )),
        ServerControl::new(),
    ));
    let snapshot = r#"{"revision":0,"projects":[],"sections":[],"sessions":[]}"#;
    let denied = router
        .clone()
        .oneshot(
            Request::put("/api/v1/library")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(snapshot))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let issued = router
        .clone()
        .oneshot(Request::get("/api/v1/session").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let cookie = issued.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let body = to_bytes(issued.into_body(), 10000).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let csrf = json["csrf_token"].as_str().unwrap();
    for (token, status) in [
        ("wrong", StatusCode::FORBIDDEN),
        (csrf, StatusCode::OK),
        (csrf, StatusCode::CONFLICT),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::put("/api/v1/library")
                    .header(header::COOKIE, &cookie)
                    .header("x-csrf-token", token)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(snapshot))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    for endpoint in ["library", "library/workspace"] {
        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/{endpoint}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = to_bytes(response.into_body(), 10000).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        if endpoint == "library" {
            assert_eq!(value["revision"], 1);
        } else {
            assert_eq!(
                value["path"],
                std::fs::canonicalize(root.join("workspace"))
                    .unwrap()
                    .to_str()
                    .unwrap()
            );
        }
    }
    drop(router);
    std::fs::remove_dir_all(root).unwrap();
}
