use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tower::ServiceExt;

use orchester_netz::{
    app_router_with_static_assets, ServerContext, ServerControl, StaticAssets,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct StaticFixture(PathBuf);

impl StaticFixture {
    fn new() -> Self {
        let nonce = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "orchester-netz-static-{}-{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&root).expect("create static fixture");
        Self(root)
    }

    fn write(&self, relative: impl AsRef<Path>, body: &str) {
        let path = self.0.join(relative.as_ref());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create static parent");
        }
        fs::write(path, body).expect("write static fixture");
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for StaticFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn test_context() -> ServerContext {
    ServerContext::new(None, ServerControl::new())
}

#[tokio::test]
async fn disabled_static_assets_keep_non_api_routes_as_typed_json_404() {
    let response = app_router_with_static_assets(test_context(), StaticAssets::Disabled)
        .oneshot(
            Request::get("/workspace/runs/42")
                .body(Body::empty())
                .expect("deep link request"),
        )
        .await
        .expect("deep link response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("not found body");
    let json: Value = serde_json::from_slice(&body).expect("not found JSON");
    assert_eq!(json["code"], "not_found");
}

#[cfg(feature = "static-files")]
#[tokio::test]
async fn configured_static_assets_serve_files_and_spa_deep_links() {
    let fixture = StaticFixture::new();
    fixture.write("index.html", "<!doctype html><title>Orchester</title>");
    fixture.write("assets/app.js", "console.log('fixture');");

    let router = app_router_with_static_assets(
        test_context(),
        StaticAssets::Directory(fixture.path().to_owned()),
    );

    let asset = router
        .clone()
        .oneshot(
            Request::get("/assets/app.js")
                .body(Body::empty())
                .expect("asset request"),
        )
        .await
        .expect("asset response");
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(asset.headers().get(header::CONTENT_TYPE).unwrap(), "text/javascript");
    assert_eq!(
        asset.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    let asset_body = to_bytes(asset.into_body(), usize::MAX)
        .await
        .expect("asset body");
    assert_eq!(&asset_body[..], b"console.log('fixture');");

    let deep_link = router
        .oneshot(
            Request::get("/workspace/runs/42")
                .body(Body::empty())
                .expect("deep link request"),
        )
        .await
        .expect("deep link response");
    assert_eq!(deep_link.status(), StatusCode::OK);
    assert_eq!(deep_link.headers().get(header::CONTENT_TYPE).unwrap(), "text/html");
    assert_eq!(
        deep_link.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-cache"
    );
    let deep_link_body = to_bytes(deep_link.into_body(), usize::MAX)
        .await
        .expect("deep link body");
    assert_eq!(&deep_link_body[..], b"<!doctype html><title>Orchester</title>");
}

#[cfg(feature = "static-files")]
#[tokio::test]
async fn configured_static_assets_keep_unknown_api_routes_as_typed_json() {
    let fixture = StaticFixture::new();
    fixture.write("index.html", "<!doctype html>");

    let response = app_router_with_static_assets(
        test_context(),
        StaticAssets::Directory(fixture.path().to_owned()),
    )
    .oneshot(
        Request::get("/api/v1/does-not-exist")
            .body(Body::empty())
            .expect("unknown API request"),
    )
    .await
    .expect("unknown API response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.headers().get(header::CONTENT_TYPE).unwrap(), "application/json");
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("unknown API body");
    let json: Value = serde_json::from_slice(&body).expect("unknown API JSON");
    assert_eq!(json["code"], "not_found");
}
