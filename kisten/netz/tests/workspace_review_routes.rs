use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{header, Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use orchester_anwendung::OrchesterPaths;
use orchester_netz::{app_router, ServerContext, ServerControl};

/// The Review tab reads the working copy through the runtime, never from the
/// browser. `GET /api/v1/workspace/review` is how it asks.

struct TempRepo(PathBuf);

impl TempRepo {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("orchester-netz-review-{label}-{nonce}"));
        fs::create_dir_all(&path).expect("temporary directory");
        let repo = Self(path);
        repo.git(&["init", "--quiet"]);
        repo.write("src/committed.txt", "committed\n");
        repo.git(&["add", "."]);
        repo.git(&[
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "user.name=Test",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ]);
        repo
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(&self.0)
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    fn write(&self, relative: &str, contents: &str) {
        let target = self.0.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("parent directory");
        }
        fs::write(target, contents).expect("file written");
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn context_for(workspace: &Path) -> ServerContext {
    ServerContext::new(
        Some(OrchesterPaths::new("private-home", workspace)),
        ServerControl::new(),
    )
}

async fn review_json(context: ServerContext) -> (StatusCode, Value) {
    let response = app_router(context)
        .oneshot(
            Request::get("/api/v1/workspace/review")
                .body(Body::empty())
                .expect("review request"),
        )
        .await
        .expect("review response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("review body");
    let json: Value = serde_json::from_slice(&body).expect("review JSON");
    (status, json)
}

#[tokio::test]
async fn review_route_reports_the_branch_and_each_paths_placement() {
    let repo = TempRepo::new("route");
    repo.write("src/staged.txt", "staged\n");
    repo.git(&["add", "src/staged.txt"]);
    repo.write("src/committed.txt", "unstaged\n");

    let (status, json) = review_json(context_for(repo.path())).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["schema_version"], 1);
    assert!(json["branch"].as_str().is_some(), "got {json}");

    let changes = json["changes"].as_array().expect("changes array");
    let staged = changes
        .iter()
        .find(|change| change["path"] == "src/staged.txt")
        .expect("staged entry");
    assert_eq!(staged["staged"], true);
    assert_eq!(staged["unstaged"], false);
    assert_eq!(staged["kind"], "added");

    let unstaged = changes
        .iter()
        .find(|change| change["path"] == "src/committed.txt")
        .expect("unstaged entry");
    assert_eq!(unstaged["staged"], false);
    assert_eq!(unstaged["unstaged"], true);
    assert_eq!(unstaged["kind"], "modified");
}

#[tokio::test]
async fn review_route_never_sends_an_absolute_path_to_the_browser() {
    let repo = TempRepo::new("paths");
    repo.write("deep/nested/file.txt", "content\n");

    let (_, json) = review_json(context_for(repo.path())).await;

    let wire = json.to_string();
    assert!(!wire.contains("private-home"));
    assert!(
        !wire.contains(repo.path().to_string_lossy().as_ref()),
        "the workspace root must not be on the wire, got {wire}"
    );
}

#[tokio::test]
async fn review_route_answers_a_plain_directory_with_an_empty_review() {
    // Orchester runs against directories that are not repositories. That is a
    // designed empty state, not a failure, so the route answers 200 with no
    // changes rather than an error the frontend would have to special-case.
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("orchester-netz-review-plain-{nonce}"));
    fs::create_dir_all(&path).expect("temporary directory");

    let (status, json) = review_json(context_for(&path)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["branch"], Value::Null);
    assert_eq!(json["changes"].as_array().map(Vec::len), Some(0));
    assert_eq!(json["branch_changes"].as_array().map(Vec::len), Some(0));

    let _ = fs::remove_dir_all(&path);
}

#[tokio::test]
async fn review_route_is_unavailable_when_no_workspace_is_selected() {
    let response = app_router(ServerContext::new(None, ServerControl::new()))
        .oneshot(
            Request::get("/api/v1/workspace/review")
                .body(Body::empty())
                .expect("review request"),
        )
        .await
        .expect("review response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn review_route_is_uncached_json_and_read_only() {
    let repo = TempRepo::new("read-only");
    repo.write("src/untracked.txt", "content\n");

    let response = app_router(context_for(repo.path()))
        .oneshot(
            Request::get("/api/v1/workspace/review")
                .body(Body::empty())
                .expect("review request"),
        )
        .await
        .expect("review response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    // Reading the review must not stage anything: asking what changed cannot be
    // allowed to change it.
    let status = Command::new("git")
        .current_dir(repo.path())
        .args(["diff", "--cached", "--name-only"])
        .output()
        .expect("git runs");
    assert!(
        status.stdout.is_empty(),
        "the review staged something: {:?}",
        String::from_utf8_lossy(&status.stdout)
    );
}

#[tokio::test]
async fn review_route_rejects_a_state_changing_method() {
    let repo = TempRepo::new("method");

    let response = app_router(context_for(repo.path()))
        .oneshot(
            Request::post("/api/v1/workspace/review")
                .body(Body::empty())
                .expect("review request"),
        )
        .await
        .expect("review response");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}
