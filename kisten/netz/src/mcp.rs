//! Explicit, user-invoked MCP connections. No background process polling or tool retries.
use crate::{
    api_error::{api_error_response, request_id_from_headers, ApiErrorCode, ApiErrorResponse},
    composer::{storage, valid_text, ComposerStore, Result},
    health::no_store_headers,
    ServerContext,
};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::HeaderMap,
    Json,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(45);
const VERSION: &str = "2025-11-25";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServerInput {
    pub name: String,
    pub transport: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub token_env: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}
#[derive(Clone, Serialize)]
pub(crate) struct Server {
    pub id: String,
    #[serde(flatten)]
    pub input: ServerInput,
}
fn env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .enumerate()
            .all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
}
impl ServerInput {
    fn validate(&self) -> Result<()> {
        if !valid_text(&self.name, 80) || self.name.chars().any(char::is_control) {
            return Err(ApiErrorCode::ValidationFailed);
        }
        match self.transport.as_str() {
            "http" => {
                let url =
                    reqwest::Url::parse(&self.url).map_err(|_| ApiErrorCode::ValidationFailed)?;
                let local = matches!(
                    url.host_str(),
                    Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
                );
                if self.url.len() > 2048
                    || !(url.scheme() == "https" || (url.scheme() == "http" && local))
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.fragment().is_some()
                    || (!self.token_env.is_empty() && !env_name(&self.token_env))
                    || !self.command.is_empty()
                    || !self.args.is_empty()
                    || !self.env.is_empty()
                {
                    return Err(ApiErrorCode::ValidationFailed);
                }
            }
            "stdio" => {
                // A direct executable and argument vector, never a shell command line.
                let program = self.command.to_ascii_lowercase();
                if !valid_text(&self.command, 2048)
                    || self.command.chars().any(char::is_control)
                    || program.ends_with(".cmd")
                    || program.ends_with(".bat")
                    || self.args.len() > 64
                    || self.args.iter().any(|a| a.len() > 4096 || a.contains('\0'))
                    || self.env.len() > 32
                    || self.env.iter().any(|(k, v)| !env_name(k) || !env_name(v))
                    || !self.url.is_empty()
                    || !self.token_env.is_empty()
                {
                    return Err(ApiErrorCode::ValidationFailed);
                }
            }
            _ => return Err(ApiErrorCode::ValidationFailed),
        }
        Ok(())
    }
}
impl ComposerStore {
    fn servers(&self) -> Result<Vec<Server>> {
        let mut q = self
            .db
            .prepare("SELECT id,data FROM mcp_servers WHERE workspace=?1 ORDER BY rowid")
            .map_err(|_| ApiErrorCode::Unavailable)?;
        let rows = q
            .query_map([&self.workspace], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|_| ApiErrorCode::Unavailable)?;
        rows.map(|row| {
            let (id, data) = row.map_err(|_| ApiErrorCode::Unavailable)?;
            Ok(Server {
                id,
                input: serde_json::from_str(&data).map_err(|_| ApiErrorCode::Unavailable)?,
            })
        })
        .collect()
    }
    fn server(&self, id: &str) -> Result<Server> {
        let data: String = self
            .db
            .query_row(
                "SELECT data FROM mcp_servers WHERE id=?1 AND workspace=?2",
                params![id, self.workspace],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| ApiErrorCode::Unavailable)?
            .ok_or(ApiErrorCode::NotFound)?;
        Ok(Server {
            id: id.into(),
            input: serde_json::from_str(&data).map_err(|_| ApiErrorCode::Unavailable)?,
        })
    }
    fn save_server(&mut self, id: Option<String>, mut input: ServerInput) -> Result<Server> {
        input.validate()?;
        input.name = input.name.trim().into();
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| ApiErrorCode::Unavailable)?;
        let data = serde_json::to_string(&input).map_err(|_| ApiErrorCode::Internal)?;
        let id = if let Some(id) = id {
            let changed = tx
                .execute(
                    "UPDATE mcp_servers SET data=?1 WHERE id=?2 AND workspace=?3",
                    params![data, id, self.workspace],
                )
                .map_err(|_| ApiErrorCode::Unavailable)?;
            if changed == 0 {
                return Err(ApiErrorCode::NotFound);
            }
            id
        } else {
            let count: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM mcp_servers WHERE workspace=?1",
                    [&self.workspace],
                    |r| r.get(0),
                )
                .map_err(|_| ApiErrorCode::Unavailable)?;
            if count >= 32 {
                return Err(ApiErrorCode::ValidationFailed);
            }
            let id = crate::schedule_store::identifier()?;
            tx.execute(
                "INSERT INTO mcp_servers(id,workspace,data) VALUES(?1,?2,?3)",
                params![id, self.workspace, data],
            )
            .map_err(|_| ApiErrorCode::Unavailable)?;
            id
        };
        tx.commit().map_err(|_| ApiErrorCode::Unavailable)?;
        Ok(Server { id, input })
    }
}
type Response<T> = std::result::Result<(HeaderMap, Json<T>), ApiErrorResponse>;
fn respond<T>(headers: &HeaderMap, result: Result<T>) -> Response<T> {
    result
        .map(|v| (no_store_headers(), Json(v)))
        .map_err(|e| api_error_response(e, request_id_from_headers(headers)))
}
pub(crate) async fn list(State(c): State<ServerContext>, h: HeaderMap) -> Response<Vec<Server>> {
    respond(&h, storage(&c, |s| s.servers()).await)
}
pub(crate) async fn create(
    State(c): State<ServerContext>,
    h: HeaderMap,
    input: std::result::Result<Json<ServerInput>, JsonRejection>,
) -> Response<Server> {
    let Json(input) = input
        .map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id_from_headers(&h)))?;
    respond(&h, storage(&c, move |s| s.save_server(None, input)).await)
}
pub(crate) async fn update(
    State(c): State<ServerContext>,
    h: HeaderMap,
    Path(id): Path<String>,
    input: std::result::Result<Json<ServerInput>, JsonRejection>,
) -> Response<Server> {
    let Json(input) = input
        .map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id_from_headers(&h)))?;
    respond(
        &h,
        storage(&c, move |s| s.save_server(Some(id), input)).await,
    )
}
pub(crate) async fn remove(
    State(c): State<ServerContext>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Response<Value> {
    respond(
        &h,
        storage(&c, move |s| {
            let n =
                s.db.execute(
                    "DELETE FROM mcp_servers WHERE id=?1 AND workspace=?2",
                    params![id, s.workspace],
                )
                .map_err(|_| ApiErrorCode::Unavailable)?;
            if n == 0 {
                Err(ApiErrorCode::NotFound)
            } else {
                Ok(json!({"deleted":true}))
            }
        })
        .await,
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Invocation {
    method: String,
    params: Value,
}
pub(crate) async fn catalog(
    State(c): State<ServerContext>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Response<Value> {
    let result = operate(c, id, None).await;
    respond(&h, result)
}
pub(crate) async fn invoke(
    State(c): State<ServerContext>,
    h: HeaderMap,
    Path(id): Path<String>,
    input: std::result::Result<Json<Invocation>, JsonRejection>,
) -> Response<Value> {
    let Json(input) = input
        .map_err(|_| api_error_response(ApiErrorCode::BadRequest, request_id_from_headers(&h)))?;
    if !matches!(
        input.method.as_str(),
        "tools/call" | "resources/read" | "prompts/get"
    ) || !input.params.is_object()
    {
        return respond(&h, Err(ApiErrorCode::ValidationFailed));
    }
    respond(&h, operate(c, id, Some(input)).await)
}
async fn operate(c: ServerContext, id: String, input: Option<Invocation>) -> Result<Value> {
    let server = storage(&c, move |s| s.server(&id)).await?;
    let workspace = c
        .paths()
        .ok_or(ApiErrorCode::Unavailable)?
        .workspace()
        .to_path_buf();
    tokio::time::timeout(TIMEOUT, async {
        let mut session = Session::connect(&server.input, &workspace).await?;
        let result = async {
            let info = session.initialize().await?;
            if let Some(input) = input {
                session.call(&input.method, input.params).await
            } else {
                let mut result =
                    json!({"serverInfo":info["serverInfo"],"tools":[],"resources":[],"prompts":[]});
                for key in ["tools", "resources", "prompts"] {
                    if info["capabilities"].get(key).is_some() {
                        result[key] = Value::Array(session.list_all(key).await?);
                    }
                }
                Ok(result)
            }
        }
        .await;
        session.close().await;
        result
    })
    .await
    .map_err(|_| ApiErrorCode::Unavailable)?
}

enum Transport {
    Http {
        client: reqwest::Client,
        url: String,
        token: Option<String>,
        session: Option<String>,
    },
    Stdio {
        child: Child,
        input: ChildStdin,
        output: BufReader<ChildStdout>,
    },
}
struct Session {
    transport: Transport,
    next_id: u64,
    version: String,
}
impl Session {
    async fn connect(input: &ServerInput, workspace: &std::path::Path) -> Result<Self> {
        input.validate()?;
        let transport = if input.transport == "http" {
            let token = if input.token_env.is_empty() {
                None
            } else {
                Some(std::env::var(&input.token_env).map_err(|_| ApiErrorCode::Unavailable)?)
            };
            Transport::Http {
                client: reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .connect_timeout(Duration::from_secs(10))
                    .timeout(TIMEOUT)
                    .build()
                    .map_err(|_| ApiErrorCode::Unavailable)?,
                url: input.url.clone(),
                token,
                session: None,
            }
        } else {
            let mut command = Command::new(&input.command);
            command
                .args(&input.args)
                .current_dir(workspace)
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            for key in [
                "PATH",
                "SystemRoot",
                "WINDIR",
                "HOME",
                "USERPROFILE",
                "TEMP",
                "TMP",
                "APPDATA",
                "LOCALAPPDATA",
                "PATHEXT",
            ] {
                if let Some(v) = std::env::var_os(key) {
                    command.env(key, v);
                }
            }
            for (key, source) in &input.env {
                command.env(
                    key,
                    std::env::var_os(source).ok_or(ApiErrorCode::Unavailable)?,
                );
            }
            #[cfg(windows)]
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: background MCP work never opens a console.
            let mut child = command.spawn().map_err(|_| ApiErrorCode::Unavailable)?;
            let input = child.stdin.take().ok_or(ApiErrorCode::Unavailable)?;
            let output = BufReader::new(child.stdout.take().ok_or(ApiErrorCode::Unavailable)?);
            Transport::Stdio {
                child,
                input,
                output,
            }
        };
        Ok(Self {
            transport,
            next_id: 0,
            version: VERSION.into(),
        })
    }
    async fn initialize(&mut self) -> Result<Value> {
        let value=self.call("initialize",json!({"protocolVersion":VERSION,"capabilities":{},"clientInfo":{"name":"Orchester","version":"0.1.8"}})).await?;
        let version = value["protocolVersion"]
            .as_str()
            .ok_or(ApiErrorCode::Unavailable)?;
        if !matches!(
            version,
            "2025-11-25" | "2025-06-18" | "2025-03-26" | "2024-11-05"
        ) || !value["serverInfo"].is_object()
            || !value["capabilities"].is_object()
        {
            return Err(ApiErrorCode::Unavailable);
        }
        self.version = version.into();
        self.exchange(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            None,
        )
        .await?;
        Ok(value)
    }
    async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next_id += 1;
        self.exchange(
            json!({"jsonrpc":"2.0","id":self.next_id,"method":method,"params":params}),
            Some(self.next_id),
        )
        .await
    }
    async fn list_all(&mut self, key: &str) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut params = json!({});
        for _ in 0..10 {
            let page = self.call(&format!("{key}/list"), params).await?;
            items.extend(
                page[key]
                    .as_array()
                    .ok_or(ApiErrorCode::Unavailable)?
                    .iter()
                    .cloned(),
            );
            if items.len() > 500 {
                return Err(ApiErrorCode::Unavailable);
            }
            match page.get("nextCursor").and_then(Value::as_str) {
                Some(cursor) => params = json!({"cursor":cursor}),
                None => return Ok(items),
            }
        }
        Err(ApiErrorCode::Unavailable)
    }
    async fn exchange(&mut self, message: Value, id: Option<u64>) -> Result<Value> {
        match &mut self.transport {
            Transport::Http {
                client,
                url,
                token,
                session,
            } => {
                let mut req = client
                    .post(url.as_str())
                    .header("Accept", "application/json, text/event-stream")
                    .header("MCP-Protocol-Version", &self.version)
                    .json(&message);
                if let Some(token) = token {
                    req = req.bearer_auth(token);
                }
                if let Some(session) = session.as_ref() {
                    req = req.header("Mcp-Session-Id", session);
                }
                let mut response = req.send().await.map_err(|_| ApiErrorCode::Unavailable)?;
                if !response.status().is_success() {
                    return Err(ApiErrorCode::Unavailable);
                }
                if message["method"] == "initialize" {
                    if let Some(value) = response.headers().get("Mcp-Session-Id") {
                        let value = value.to_str().map_err(|_| ApiErrorCode::Unavailable)?;
                        if value.len() > 1024
                            || value.is_empty()
                            || !value.bytes().all(|c| c.is_ascii_graphic())
                        {
                            return Err(ApiErrorCode::Unavailable);
                        }
                        *session = Some(value.into());
                    }
                }
                let Some(id) = id else { return Ok(Value::Null) };
                let sse = response
                    .headers()
                    .get("Content-Type")
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.starts_with("text/event-stream"));
                let mut bytes = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| ApiErrorCode::Unavailable)?
                {
                    if bytes.len() + chunk.len() > MAX_BYTES {
                        return Err(ApiErrorCode::Unavailable);
                    }
                    bytes.extend_from_slice(&chunk);
                    if sse {
                        if let Some(value) = sse_result(&bytes, id)? {
                            return Ok(value);
                        }
                    }
                }
                if sse {
                    return Err(ApiErrorCode::Unavailable);
                }
                rpc_result(
                    serde_json::from_slice(&bytes).map_err(|_| ApiErrorCode::Unavailable)?,
                    id,
                )?
                .ok_or(ApiErrorCode::Unavailable)
            }
            Transport::Stdio { input, output, .. } => {
                let mut wire = serde_json::to_vec(&message).map_err(|_| ApiErrorCode::Internal)?;
                wire.push(b'\n');
                input
                    .write_all(&wire)
                    .await
                    .map_err(|_| ApiErrorCode::Unavailable)?;
                input.flush().await.map_err(|_| ApiErrorCode::Unavailable)?;
                let Some(id) = id else { return Ok(Value::Null) };
                for _ in 0..256 {
                    let mut line = Vec::new();
                    loop {
                        let buffer = output
                            .fill_buf()
                            .await
                            .map_err(|_| ApiErrorCode::Unavailable)?;
                        if buffer.is_empty() {
                            return Err(ApiErrorCode::Unavailable);
                        }
                        let end = buffer.iter().position(|c| *c == b'\n').map(|i| i + 1);
                        let count = end.unwrap_or(buffer.len());
                        if line.len() + count > MAX_BYTES {
                            return Err(ApiErrorCode::Unavailable);
                        }
                        line.extend_from_slice(&buffer[..count]);
                        output.consume(count);
                        if end.is_some() {
                            break;
                        }
                    }
                    let value: Value =
                        serde_json::from_slice(&line).map_err(|_| ApiErrorCode::Unavailable)?;
                    if value.get("method").is_some() && value.get("id").is_some() {
                        let answer = if value["method"] == "ping" {
                            json!({"jsonrpc":"2.0","id":value["id"],"result":{}})
                        } else {
                            json!({"jsonrpc":"2.0","id":value["id"],"error":{"code":-32601,"message":"Unsupported client method"}})
                        };
                        let mut wire =
                            serde_json::to_vec(&answer).map_err(|_| ApiErrorCode::Internal)?;
                        wire.push(b'\n');
                        input
                            .write_all(&wire)
                            .await
                            .map_err(|_| ApiErrorCode::Unavailable)?;
                        input.flush().await.map_err(|_| ApiErrorCode::Unavailable)?;
                    } else if let Some(value) = rpc_result(value, id)? {
                        return Ok(value);
                    }
                }
                Err(ApiErrorCode::Unavailable)
            }
        }
    }
    async fn close(&mut self) {
        match &mut self.transport {
            Transport::Stdio { child, .. } => {
                let _ = child.kill().await;
            }
            Transport::Http {
                client,
                url,
                token,
                session: Some(session),
            } => {
                let mut req = client
                    .delete(url.as_str())
                    .header("Mcp-Session-Id", session.as_str())
                    .header("MCP-Protocol-Version", &self.version)
                    .timeout(Duration::from_secs(2));
                if let Some(token) = token {
                    req = req.bearer_auth(token);
                }
                let _ = req.send().await;
            }
            _ => {}
        }
    }
}
fn rpc_result(value: Value, id: u64) -> Result<Option<Value>> {
    if value["jsonrpc"] != "2.0" {
        return Err(ApiErrorCode::Unavailable);
    }
    if value["id"].as_u64() != Some(id) || value.get("method").is_some() {
        return Ok(None);
    }
    if value.get("error").is_some() {
        return Err(ApiErrorCode::RuntimeError);
    }
    value
        .get("result")
        .cloned()
        .map(Some)
        .ok_or(ApiErrorCode::Unavailable)
}
fn sse_result(bytes: &[u8], id: u64) -> Result<Option<Value>> {
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) if e.error_len().is_none() => return Ok(None),
        Err(_) => return Err(ApiErrorCode::Unavailable),
    };
    let normalized = text.replace("\r\n", "\n");
    for block in normalized
        .split_inclusive("\n\n")
        .filter(|b| b.ends_with("\n\n"))
    {
        let data = block
            .lines()
            .filter_map(|l| {
                l.strip_prefix("data:")
                    .map(|v| v.strip_prefix(' ').unwrap_or(v))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !data.is_empty() {
            if let Some(v) = rpc_result(
                serde_json::from_str(&data).map_err(|_| ApiErrorCode::Unavailable)?,
                id,
            )? {
                return Ok(Some(v));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_handles_notifications_split_utf8_and_multiline_data() {
        let data=b"data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/message\"}\r\n\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":2,\n data: ignored\ndata: \"result\":{\"tools\":[]}}\n\n";
        assert_eq!(sse_result(data, 2).unwrap(), Some(json!({"tools":[]})));
        assert!(sse_result(b"data: {\"jsonrpc\":\"2.0\",\"id\":2", 2)
            .unwrap()
            .is_none());
        assert!(sse_result(b"data: \xe4\xb8", 2).unwrap().is_none());
    }
    #[test]
    fn definitions_reject_secrets_in_urls_and_shell_wrappers() {
        let mut input: ServerInput = serde_json::from_value(
            json!({"name":"Test","transport":"http","url":"http://127.0.0.1:123/mcp"}),
        )
        .unwrap();
        assert!(input.validate().is_ok());
        input.url = "https://user:secret@example.com/mcp".into();
        assert!(input.validate().is_err());
        input.url = "http://example.com/mcp".into();
        assert!(input.validate().is_err());
        input.transport = "stdio".into();
        input.url.clear();
        input.command = "npx.cmd".into();
        assert!(input.validate().is_err());
        input.command = "node.exe".into();
        assert!(input.validate().is_ok());
    }
}
