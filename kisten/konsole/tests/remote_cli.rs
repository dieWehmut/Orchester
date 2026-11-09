use std::{path::PathBuf, process::Stdio, time::Duration};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn root() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "orchester-remote-cli-{}-{nanos}",
        std::process::id()
    ))
}

#[tokio::test]
async fn serves_only_remote_api_and_assets_with_local_pairing_controls() {
    let root = root();
    let assets = root.join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(assets.join("index.html"), "mobile fixture").unwrap();
    let manifests = root.join("manifeste");
    std::fs::create_dir(&manifests).unwrap();
    #[cfg(unix)]
    let (command, args) = {
        use std::os::unix::fs::PermissionsExt;
        let file = root.join("fixture.sh");
        std::fs::write(
            &file,
            "#!/bin/sh\nprintf '%s\\n' '{\"type\":\"result\",\"text\":\"fixture done\"}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
        (file.to_string_lossy().into_owned(), Vec::<String>::new())
    };
    #[cfg(windows)]
    let (command, args) = {
        let file = root.join("fixture.ps1");
        std::fs::write(
            &file,
            "Write-Output '{\"type\":\"result\",\"text\":\"fixture done\"}'",
        )
        .unwrap();
        (
            "powershell.exe".to_owned(),
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-File".into(),
                file.to_string_lossy().into_owned(),
            ],
        )
    };
    std::fs::write(manifests.join("fixture.toml"),format!("name = \"fixture\"\ncommand = {}\nargs = {}\n[parse]\ndiscriminator = \"type\"\n[parse.map]\nresult = {{event=\"result\",text=\"text\"}}\n",serde_json::to_string(&command).unwrap(),serde_json::to_string(&args).unwrap())).unwrap();

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_orchester"))
        .env("ORCHESTER_HOME", root.join("home"))
        .args(["remote", "--port", "0", "--json", "--workspace"])
        .arg(&root)
        .arg("--assets")
        .arg(&assets)
        .args(["--allow-origin", "http://localhost:4175"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let ready: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(10), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let base = ready["address"].as_str().unwrap();
    assert!(base.starts_with("http://127.0.0.1:"));
    let http = reqwest::Client::new();
    assert_eq!(
        http.get(base).send().await.unwrap().text().await.unwrap(),
        "mobile fixture"
    );
    assert_eq!(
        http.get(format!("{base}/api/v1/bootstrap"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        http.get(format!("{base}/remote/v1/info"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let preflight = http
        .request(reqwest::Method::OPTIONS, format!("{base}/remote/v1/pair"))
        .header("origin", "http://localhost:4175")
        .header("access-control-request-method", "POST")
        .send()
        .await
        .unwrap();
    assert_eq!(
        preflight.headers()["access-control-allow-origin"],
        "http://localhost:4175"
    );
    let denied = http
        .request(reqwest::Method::OPTIONS, format!("{base}/remote/v1/pair"))
        .header("origin", "http://unexpected.example")
        .header("access-control-request-method", "POST")
        .send()
        .await
        .unwrap();
    assert!(!denied.headers().contains_key("access-control-allow-origin"));
    let paired: Value = serde_json::from_str(
        &http
            .post(format!("{base}/remote/v1/pair"))
            .header("content-type", "application/json")
            .body(json!({"code":ready["pairing_code"],"device_name":"CLI acceptance"}).to_string())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    let token = paired["token"].as_str().unwrap();
    let started: Value = serde_json::from_str(
        &http
            .post(format!("{base}/remote/v1/runs"))
            .bearer_auth(token)
            .header("content-type", "application/json")
            .body(
                json!({"agent":"fixture","prompt":"remote CLI acceptance","idempotency_key":"once"})
                    .to_string(),
            )
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    let run = started["id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let page: Value = serde_json::from_str(
                &http
                    .get(format!("{base}/remote/v1/runs/{run}/events"))
                    .bearer_auth(token)
                    .send()
                    .await
                    .unwrap()
                    .text()
                    .await
                    .unwrap(),
            )
            .unwrap();
            if page["state"] == "completed" {
                assert!(page["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["event"]["text"] == "fixture done"));
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut input = child.stdin.take().unwrap();
    input
        .write_all(format!("revoke {}\n", paired["device_id"].as_str().unwrap()).as_bytes())
        .await
        .unwrap();
    let revoked: Value = serde_json::from_str(
        &tokio::time::timeout(Duration::from_secs(5), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(revoked["revoked"], true);
    assert_eq!(
        http.get(format!("{base}/remote/v1/info"))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    input.write_all(b"quit\n").await.unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_wildcard_origin_before_opening_a_listener() {
    let root = root();
    std::fs::create_dir_all(&root).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_orchester"))
        .env("ORCHESTER_HOME", root.join("home"))
        .args(["remote", "--workspace"])
        .arg(&root)
        .args(["--allow-origin", "*"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("exact HTTP(S) origin"));
    std::fs::remove_dir_all(root).unwrap();
}
