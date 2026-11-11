//! Explicitly enabled remote delegate API. Local cookie routes are never exposed.
use crate::ServerContext;
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random() -> std::io::Result<String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(std::io::Error::other)?;
    Ok(b.iter().map(|v| format!("{v:02x}")).collect())
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
#[derive(Clone)]
pub struct RemoteControl(Arc<Inner>);
struct Inner {
    context: ServerContext,
    workspace: PathBuf,
    allowed_roots: Vec<PathBuf>,
    default_project_id: String,
    projects: Mutex<HashMap<String, Project>>,
    database: Mutex<rusqlite::Connection>,
    host_id: String,
    stopping: AtomicBool,
    _lease: std::fs::File,
    auth: Mutex<Auth>,
    runs: Mutex<HashMap<String, Run>>,
    terminals: terminal::Terminals,
}
#[derive(Clone, Serialize, Deserialize, Default)]
struct Auth {
    #[serde(skip)]
    attempts: u32,
    #[serde(skip)]
    code: String,
    #[serde(skip)]
    expires: u64,
    tokens: HashMap<String, Device>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RemoteDevice {
    pub device_id: String,
    pub device_name: String,
    pub expires_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
struct Device {
    info: RemoteDevice,
}

#[derive(Serialize, Deserialize)]
struct Run {
    #[serde(default)]
    bytes: usize,
    summary: Value,
    events: Vec<Value>,
    #[serde(skip)]
    cancel: CancellationToken,
    fingerprint: String,
    idempotency_key: Option<String>,
}
impl RemoteControl {
    pub fn new(context: ServerContext, workspace: PathBuf) -> std::io::Result<Self> {
        Self::with_allowed_roots(context, workspace, Vec::new())
    }
    pub fn with_allowed_roots(
        context: ServerContext,
        workspace: PathBuf,
        roots: Vec<PathBuf>,
    ) -> std::io::Result<Self> {
        let workspace = workspace.canonicalize()?;
        let mut allowed_roots = vec![workspace.clone()];
        for root in roots {
            if !root.is_absolute() {
                return Err(std::io::Error::other("allowed root must be absolute"));
            }
            let actual = root.canonicalize()?;
            if !actual.is_dir() {
                return Err(std::io::Error::other("allowed root must be a directory"));
            }
            if !allowed_roots.contains(&actual) {
                allowed_roots.push(actual);
            }
        }
        if workspace.to_str().is_none() || allowed_roots.iter().any(|root| root.to_str().is_none())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "remote paths must be valid UTF-8",
            ));
        }
        if !workspace.is_dir() {
            return Err(std::io::Error::other("workspace must be a directory"));
        }
        let paths = context
            .paths()
            .ok_or_else(|| std::io::Error::other("remote requires host paths"))?;
        let db = orchester_laufzeit::harness::run_store::open_private_database(
            &paths.state_root().join("remote.db"),
        )
        .map_err(std::io::Error::other)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS remote_state (key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS remote_runs (workspace TEXT NOT NULL, id TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY(workspace,id))",
        )
        .map_err(std::io::Error::other)?;
        // SQLite protects transactions; this lease also prevents two supervisors
        // from concurrently accepting work and overwriting in-memory snapshots.
        let lease_path = paths.state_root().join("remote.lock.db");
        let _lock_db = orchester_laufzeit::harness::run_store::open_private_database(&lease_path)
            .map_err(std::io::Error::other)?;
        let lease = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lease_path)?;
        fs2::FileExt::try_lock_exclusive(&lease).map_err(|_| {
            std::io::Error::other("another remote supervisor owns this state directory")
        })?;
        let key = workspace.to_string_lossy().to_string();
        let mut auth: Auth = read_value(&db, "auth:host")?
            .or(read_value(&db, &format!("auth:{key}"))?)
            .unwrap_or_default();
        write_value(&db, "auth:host", &auth)?;
        let default_project = Project::from_path(workspace.clone());
        let default_project_id = default_project.id.clone();
        let mut projects: HashMap<String, Project> =
            read_value(&db, "projects")?.unwrap_or_default();
        projects.retain(|_, project| {
            project.path.canonicalize().ok().is_some_and(|path| {
                path == project.path && allowed_roots.iter().any(|root| path.starts_with(root))
            })
        });
        projects.insert(default_project.id.clone(), default_project);
        write_value(&db, "projects", &projects)?;
        auth.code = random()?[..12].to_owned();
        auth.expires = now() + 600;
        let host_id: String = read_value(&db, "host_id")?.unwrap_or(random()?);
        write_value(&db, "host_id", &host_id)?;
        let mut runs: HashMap<String, Run> = HashMap::new();
        {
            let mut statement = db
                .prepare("SELECT workspace,id,value FROM remote_runs")
                .map_err(std::io::Error::other)?;
            let rows = statement
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(std::io::Error::other)?;
            for row in rows {
                let (run_workspace, id, value) = row.map_err(std::io::Error::other)?;
                let mut run: Run = serde_json::from_str(&value).map_err(std::io::Error::other)?;
                let project_id = run.summary["project_id"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| hash(&run_workspace));
                if let Some(project) = projects.get(&project_id) {
                    run.summary["project_id"] = json!(project.id);
                    run.summary["workspace"] = json!(project.path);
                    runs.insert(id, run);
                }
            }
        }
        for run in runs.values_mut() {
            if run.summary["state"] == "running" {
                run.summary["state"] = json!("interrupted");
            }
        }
        for (id, run) in &runs {
            write_run(
                &db,
                run.summary["workspace"].as_str().unwrap_or(&key),
                id,
                run,
            )?;
        }
        Ok(Self(Arc::new(Inner {
            context,
            workspace,
            allowed_roots,
            default_project_id,
            projects: Mutex::new(projects),
            database: Mutex::new(db),
            host_id,
            stopping: AtomicBool::new(false),
            _lease: lease,
            auth: Mutex::new(auth),
            runs: Mutex::new(runs),
            terminals: terminal::Terminals::default(),
        })))
    }
    fn save_run(&self, id: &str, run: &Run) -> std::io::Result<()> {
        write_run(
            &self.0.database.lock().unwrap(),
            run.summary["workspace"]
                .as_str()
                .unwrap_or(&self.0.workspace.to_string_lossy()),
            id,
            run,
        )
    }
    pub fn pairing_code(&self) -> String {
        self.0.auth.lock().unwrap().code.clone()
    }
    pub fn pairing_expires_at(&self) -> u64 {
        self.0.auth.lock().unwrap().expires
    }
    pub fn rotate_pairing_code(&self) -> std::io::Result<String> {
        let mut a = self.0.auth.lock().unwrap();
        a.code = random()?[..12].to_owned();
        a.expires = now() + 600;
        a.attempts = 0;
        Ok(a.code.clone())
    }
    pub fn disable_pairing(&self) {
        self.0.auth.lock().unwrap().expires = 0;
    }
    pub fn devices(&self) -> Vec<RemoteDevice> {
        self.0
            .auth
            .lock()
            .unwrap()
            .tokens
            .values()
            .map(|d| d.info.clone())
            .collect()
    }
    pub fn revoke_device(&self, id: &str) -> std::io::Result<bool> {
        let mut a = self.0.auth.lock().unwrap();
        let previous = a.clone();
        let old = a.tokens.len();
        let revoked: Vec<_> = a
            .tokens
            .iter()
            .filter(|(_, d)| d.info.device_id == id)
            .map(|(key, _)| key.clone())
            .collect();
        a.tokens.retain(|_, d| d.info.device_id != id);
        if let Err(error) = self.save("auth", &*a) {
            *a = previous;
            return Err(error);
        }
        let removed = old != a.tokens.len();
        drop(a);
        for key in revoked {
            self.0.terminals.revoke(&key);
        }
        Ok(removed)
    }
    fn save<T: Serialize>(&self, kind: &str, value: &T) -> std::io::Result<()> {
        write_value(
            &self.0.database.lock().unwrap(),
            &format!("{kind}:host"),
            value,
        )
    }
    /// Stop delegates and drain terminal IO before listener shutdown.
    pub async fn shutdown(&self) {
        self.0.stopping.store(true, Ordering::SeqCst);
        self.0.terminals.shutdown();
        {
            let runs = self.0.runs.lock().unwrap();
            for run in runs.values() {
                run.cancel.cancel();
            }
        }
        for _ in 0..100 {
            if !self
                .0
                .runs
                .lock()
                .unwrap()
                .values()
                .any(|r| r.summary["state"] == "running")
                && !self.0.terminals.running()
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }
    pub fn router(&self) -> Router {
        Router::new()
            .route("/remote/v1/pair", post(pair))
            .route("/remote/v1/info", get(info))
            .route("/remote/v1/agents", get(agents))
            .route("/remote/v1/agents/status", get(agent_status))
            .route("/remote/v1/projects", get(list_projects).post(add_project))
            .route("/remote/v1/folders", get(folders))
            .route("/remote/v1/runs", get(list).post(start))
            .route("/remote/v1/runs/{id}", get(snapshot).delete(remove))
            .route("/remote/v1/runs/{id}/events", get(events))
            .route("/remote/v1/runs/{id}/cancel", post(cancel))
            .route("/remote/v1/device", axum::routing::delete(revoke))
            .route(
                "/remote/v1/terminals",
                get(terminal::list).post(terminal::create),
            )
            .route(
                "/remote/v1/terminals/{id}",
                get(terminal::get).delete(terminal::close),
            )
            .route("/remote/v1/terminals/{id}/ticket", post(terminal::ticket))
            .route("/remote/v1/terminals/{id}/ws", get(terminal::socket))
            .layer(DefaultBodyLimit::max(65536))
            .layer(axum::middleware::map_response(
                |mut response: axum::response::Response| async move {
                    response.headers_mut().insert(
                        axum::http::header::CACHE_CONTROL,
                        axum::http::HeaderValue::from_static("no-store"),
                    );
                    response
                },
            ))
            .with_state(self.clone())
    }
    fn authorize(&self, h: &HeaderMap) -> Result<(), StatusCode> {
        self.authorize_key(h).map(|_| ())
    }
    fn authorize_key(&self, h: &HeaderMap) -> Result<String, StatusCode> {
        let t = h
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let key = hash(t);
        if self.key_authorized(&key) {
            Ok(key)
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    }
    fn key_authorized(&self, key: &str) -> bool {
        self.0
            .auth
            .lock()
            .unwrap()
            .tokens
            .get(key)
            .is_some_and(|d| d.info.expires_at > now())
    }
}
async fn info(State(s): State<RemoteControl>, h: HeaderMap) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    Ok(Json(
        json!({"schema_version":2,"name":std::env::var("COMPUTERNAME").or_else(|_|std::fs::read_to_string("/etc/hostname")).unwrap_or_else(|_|"Orchester".into()).trim(),"host_id":s.0.host_id,"workspace":s.0.workspace,"default_project_id":s.0.default_project_id,"allowed_roots":s.0.allowed_roots}),
    ))
}
async fn agents(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<ProjectQuery>,
) -> Result<Json<Value>, StatusCode> {
    s.authorize(&h)?;
    let project = s.project(q.project_id.as_deref())?;
    s.validate_project(&project)?;
    let registry = s.project_registry(&project);
    let agents: Vec<_> =
        registry
            .availability()
            .iter()
            .map(|a| json!({"name":a.name,"available":!a.is_missing(),"supports_resume":registry.get(&a.name).is_some_and(|adapter| adapter.capabilities().supports_resume)}))
            .collect();
    Ok(Json(json!({"agents":agents})))
}

async fn agent_status(
    State(s): State<RemoteControl>,
    h: HeaderMap,
) -> Result<Json<orchester_protokoll::AgentFleetSnapshotDto>, StatusCode> {
    s.authorize(&h)?;
    let _ = s.0.context.refresh_agent_processes().await;
    // Process collection may wait for a bounded worker. A device revoked while
    // that worker is running must not receive the resulting host snapshot.
    s.authorize(&h)?;
    s.0.context
        .agent_status_store()
        .snapshot()
        .map(Json)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

fn read_value<T: serde::de::DeserializeOwned>(
    db: &rusqlite::Connection,
    key: &str,
) -> std::io::Result<Option<T>> {
    use rusqlite::OptionalExtension;
    let value: Option<String> = db
        .query_row("SELECT value FROM remote_state WHERE key=?1", [key], |r| {
            r.get(0)
        })
        .optional()
        .map_err(std::io::Error::other)?;
    value
        .map(|v| serde_json::from_str(&v).map_err(std::io::Error::other))
        .transpose()
}
fn write_value<T: Serialize>(
    db: &rusqlite::Connection,
    key: &str,
    value: &T,
) -> std::io::Result<()> {
    let value = serde_json::to_string(value).map_err(std::io::Error::other)?;
    db.execute("INSERT INTO remote_state (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",rusqlite::params![key,value]).map_err(std::io::Error::other)?;
    Ok(())
}

mod projects;
use projects::{add_project, folders, list_projects, Project, ProjectQuery};
mod auth;
mod runs;
mod terminal;
use auth::{pair, revoke};
use runs::{cancel, events, list, remove, snapshot, start};

fn write_run(
    db: &rusqlite::Connection,
    workspace: &str,
    id: &str,
    run: &Run,
) -> std::io::Result<()> {
    let value = serde_json::to_string(run).map_err(std::io::Error::other)?;
    db.execute("INSERT INTO remote_runs(workspace,id,value) VALUES (?1,?2,?3) ON CONFLICT(workspace,id) DO UPDATE SET value=excluded.value",rusqlite::params![workspace,id,value]).map_err(std::io::Error::other)?;
    Ok(())
}
