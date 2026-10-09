use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};
use tower::ServiceExt;

#[tokio::test]
async fn plugin_catalog_is_explicitly_unavailable_without_workspace() {
    let response = app_router(ServerContext::new(None, ServerControl::new()))
        .oneshot(Request::get("/api/v1/plugins").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn catalog_and_mutation_validation_do_not_execute_arbitrary_packages() {
    let root = std::env::temp_dir().join(format!("orchester-plugin-api-{}", std::process::id()));
    let context = ServerContext::new(
        Some(OrchesterPaths::new(root.join("home"), &root)),
        ServerControl::new(),
    );
    let response = app_router(context.clone())
        .oneshot(Request::get("/api/v1/plugins").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(body["plugins"].as_array().unwrap().len(), 3);
    for plugin in body["plugins"].as_array().unwrap() {
        assert_eq!(plugin["installed"], false);
        assert_eq!(plugin["removable"], false);
    }
    for (payload, status) in [
        (r#"{"name":"../escape"}"#, StatusCode::UNPROCESSABLE_ENTITY),
        (
            r#"{"name":"codex","script":"echo nope"}"#,
            StatusCode::BAD_REQUEST,
        ),
        (
            r#"{"name":"untrusted-package"}"#,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let response = app_router(context.clone())
            .oneshot(
                Request::post("/api/v1/plugins")
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    assert!(!root.exists());
}
