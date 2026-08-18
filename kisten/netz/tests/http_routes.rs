use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};

fn test_context() -> ServerContext {
    ServerContext::new(None, ServerControl::new())
}

#[tokio::test]
async fn health_route_returns_uncached_json_with_the_typed_payload() {
    let response = app_router(test_context())
        .oneshot(
            Request::get("/api/v1/health")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("health body");
    let json: Value = serde_json::from_slice(&body).expect("health JSON");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "orchester");
    assert_eq!(json["schema_version"], 1);
}

#[tokio::test]
async fn unknown_api_route_returns_not_found() {
    let response = app_router(test_context())
        .oneshot(
            Request::get("/api/v1/does-not-exist")
                .body(Body::empty())
                .expect("unknown request"),
        )
        .await
        .expect("unknown response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bootstrap_route_returns_the_safe_context_snapshot() {
    let context = ServerContext::new(
        Some(OrchesterPaths::new("private-home", "visible-project")),
        ServerControl::new(),
    );
    let response = app_router(context)
        .oneshot(
            Request::get("/api/v1/bootstrap")
                .body(Body::empty())
                .expect("bootstrap request"),
        )
        .await
        .expect("bootstrap response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("bootstrap body");
    let json: Value = serde_json::from_slice(&body).expect("bootstrap JSON");
    assert_eq!(
        json["workspace"],
        serde_json::json!({
            "selected": true,
            "name": "visible-project",
        })
    );
    assert!(!json.to_string().contains("private-home"));
}

#[tokio::test]
async fn routes_generate_a_request_id_when_the_client_does_not_send_one() {
    let response = app_router(test_context())
        .oneshot(
            Request::get("/api/v1/health")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");

    let value = response
        .headers()
        .get("x-request-id")
        .expect("generated request id")
        .to_str()
        .expect("request id text");
    assert_eq!(value.len(), 36);
    assert_eq!(value.as_bytes()[8], b'-');
    assert_eq!(value.as_bytes()[13], b'-');
    assert_eq!(value.as_bytes()[18], b'-');
    assert_eq!(value.as_bytes()[23], b'-');
}

#[tokio::test]
async fn routes_propagate_a_client_request_id() {
    let response = app_router(test_context())
        .oneshot(
            Request::get("/api/v1/health")
                .header("x-request-id", "browser-request-123")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");

    assert_eq!(
        response.headers().get("x-request-id").unwrap(),
        "browser-request-123"
    );
}

#[tokio::test]
async fn session_bootstrap_sets_an_http_only_cookie_and_returns_only_csrf_json() {
    let router = app_router(test_context());
    let response = router
        .oneshot(
            Request::get("/api/v1/session")
                .body(Body::empty())
                .expect("session request"),
        )
        .await
        .expect("session response");

    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("session cookie")
        .to_str()
        .expect("cookie text");
    assert!(cookie.starts_with("orchester_session="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(cookie.contains("Path=/"));

    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("session body");
    let json: Value = serde_json::from_slice(&body).expect("session JSON");
    assert!(json["csrf_token"].as_str().is_some());
    assert!(json["expires_at"].as_u64().unwrap_or_default() > 0);
    assert!(!json.to_string().contains("session_cookie"));
}

#[tokio::test]
async fn revoke_requires_csrf_and_accepts_the_issued_cookie_once() {
    let router = app_router(test_context());
    let denied = router
        .clone()
        .oneshot(
            Request::post("/api/v1/session/revoke")
                .body(Body::empty())
                .expect("revoke request"),
        )
        .await
        .expect("revoke response");
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let issued = router
        .clone()
        .oneshot(
            Request::get("/api/v1/session")
                .body(Body::empty())
                .expect("session request"),
        )
        .await
        .expect("session response");
    let set_cookie = issued
        .headers()
        .get(header::SET_COOKIE)
        .expect("session cookie")
        .to_str()
        .expect("cookie text")
        .to_owned();
    let cookie = set_cookie
        .split(';')
        .next()
        .expect("cookie pair")
        .to_owned();
    let body = to_bytes(issued.into_body(), usize::MAX)
        .await
        .expect("session body");
    let csrf = serde_json::from_slice::<Value>(&body).expect("session JSON")["csrf_token"]
        .as_str()
        .expect("csrf token")
        .to_owned();

    let invalid = router
        .clone()
        .oneshot(
            Request::post("/api/v1/session/revoke")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", "wrong")
                .body(Body::empty())
                .expect("invalid revoke request"),
        )
        .await
        .expect("invalid revoke response");
    assert_eq!(invalid.status(), StatusCode::FORBIDDEN);

    let accepted = router
        .clone()
        .oneshot(
            Request::post("/api/v1/session/revoke")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .expect("valid revoke request"),
        )
        .await
        .expect("valid revoke response");
    assert_eq!(accepted.status(), StatusCode::NO_CONTENT);

    let repeated = router
        .oneshot(
            Request::post("/api/v1/session/revoke")
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .expect("repeated revoke request"),
        )
        .await
        .expect("repeated revoke response");
    assert_eq!(repeated.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn fragment_exchange_consumes_a_registered_token_once() {
    let context = test_context();
    context
        .provision_fragment_token("fragment-for-browser")
        .expect("fragment token");
    let router = app_router(context);

    let exchanged = router
        .clone()
        .oneshot(
            Request::post("/api/v1/auth/fragment")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schema_version":1,"fragment_token":"fragment-for-browser"}"#,
                ))
                .expect("fragment exchange request"),
        )
        .await
        .expect("fragment exchange response");
    assert_eq!(exchanged.status(), StatusCode::OK);
    assert!(exchanged.headers().get(header::SET_COOKIE).is_some());

    let repeated = router
        .clone()
        .oneshot(
            Request::post("/api/v1/auth/fragment")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schema_version":1,"fragment_token":"fragment-for-browser"}"#,
                ))
                .expect("repeated fragment exchange request"),
        )
        .await
        .expect("repeated fragment exchange response");
    assert_eq!(repeated.status(), StatusCode::UNAUTHORIZED);

    let query = router
        .oneshot(
            Request::get("/api/v1/auth/fragment?fragment_token=fragment-for-browser")
                .body(Body::empty())
                .expect("query fragment request"),
        )
        .await
        .expect("query fragment response");
    assert_eq!(query.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn fragment_exchange_rejects_schema_mismatch_without_consuming_token() {
    let context = test_context();
    context
        .provision_fragment_token("schema-token")
        .expect("fragment token");
    let router = app_router(context);

    let invalid_schema = router
        .clone()
        .oneshot(
            Request::post("/api/v1/auth/fragment")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schema_version":99,"fragment_token":"schema-token"}"#,
                ))
                .expect("invalid schema request"),
        )
        .await
        .expect("invalid schema response");
    assert_eq!(invalid_schema.status(), StatusCode::BAD_REQUEST);

    let valid = router
        .oneshot(
            Request::post("/api/v1/auth/fragment")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"schema_version":1,"fragment_token":"schema-token"}"#,
                ))
                .expect("valid schema request"),
        )
        .await
        .expect("valid schema response");
    assert_eq!(valid.status(), StatusCode::OK);
}
