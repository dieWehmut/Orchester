//! Device-owned interactive terminals, separate from delegate runs and history.
use super::*;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query};
use axum::response::Response;
use std::time::Duration;
mod runtime;
use runtime::Terminal;

#[derive(Clone, Serialize)]
pub(super) struct TerminalInfo {
    id: String,
    project_id: String,
    workspace: PathBuf,
    shell: String,
    cols: u16,
    rows: u16,
    state: String,
    created_at: u64,
    exit_code: Option<u32>,
}
#[derive(Default)]
pub(super) struct Terminals {
    sessions: Mutex<HashMap<String, Arc<Terminal>>>,
    tickets: Mutex<HashMap<String, Ticket>>,
}
struct Ticket {
    id: String,
    owner: String,
    expires: u64,
    after: u64,
}
impl Terminals {
    pub fn shutdown(&self) {
        for session in self.sessions.lock().unwrap().values() {
            session.close.store(true, Ordering::SeqCst);
        }
        self.tickets.lock().unwrap().clear();
    }
    pub fn revoke(&self, owner: &str) {
        for session in self
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|session| session.owner == owner)
        {
            session.close.store(true, Ordering::SeqCst);
            session.changed.notify_one();
        }
        self.tickets
            .lock()
            .unwrap()
            .retain(|_, ticket| ticket.owner != owner);
    }
    pub fn running(&self) -> bool {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .any(|session| !session.workers_done.load(Ordering::SeqCst))
    }
    fn find(&self, id: &str, owner: &str) -> Result<Arc<Terminal>, StatusCode> {
        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .filter(|session| session.owner == owner)
            .cloned()
            .ok_or(StatusCode::NOT_FOUND)
    }
}
impl Drop for Terminals {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn valid_size(cols: u16, rows: u16) -> bool {
    (2..=500).contains(&cols) && (2..=300).contains(&rows)
}
fn default_cols() -> u16 {
    80
}
fn default_rows() -> u16 {
    24
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Create {
    project_id: Option<String>,
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
}
pub(super) async fn create(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Json(input): Json<Create>,
) -> Result<Json<TerminalInfo>, StatusCode> {
    let owner = s.authorize_key(&h)?;
    if !valid_size(input.cols, input.rows) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let project = s.project(input.project_id.as_deref())?;
    let host = s.clone();
    let terminal = tokio::task::spawn_blocking(move || {
        let mut sessions = host.0.terminals.sessions.lock().unwrap();
        if host.0.stopping.load(Ordering::SeqCst) {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        if !host.key_authorized(&owner) {
            return Err(StatusCode::UNAUTHORIZED);
        }
        host.validate_project(&project)?;
        if sessions
            .values()
            .filter(|session| session.snapshot().state == "running")
            .count()
            >= 4
        {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        // Ended history is only in memory, capped independently of live shells.
        if sessions.len() >= 64 {
            let oldest = sessions
                .iter()
                .filter(|(_, session)| session.snapshot().state != "running")
                .min_by_key(|(_, session)| session.snapshot().created_at)
                .map(|(id, _)| id.clone());
            if let Some(id) = oldest {
                sessions.remove(&id);
                host.0
                    .terminals
                    .tickets
                    .lock()
                    .unwrap()
                    .retain(|_, ticket| ticket.id != id);
            }
        }
        let session = Terminal::spawn(owner, project, input.cols, input.rows)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let info = session.snapshot();
        sessions.insert(info.id.clone(), session);
        Ok(info)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(Json(terminal))
}
pub(super) async fn list(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Query(query): Query<ProjectQuery>,
) -> Result<Json<Value>, StatusCode> {
    let owner = s.authorize_key(&h)?;
    let mut terminals: Vec<_> =
        s.0.terminals
            .sessions
            .lock()
            .unwrap()
            .values()
            .filter(|session| session.owner == owner)
            .map(|session| session.snapshot())
            .filter(|info| {
                query
                    .project_id
                    .as_ref()
                    .map_or(true, |project_id| &info.project_id == project_id)
            })
            .collect();
    terminals.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(Json(json!({"terminals":terminals})))
}
pub(super) async fn get(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<TerminalInfo>, StatusCode> {
    Ok(Json(
        s.0.terminals.find(&id, &s.authorize_key(&h)?)?.snapshot(),
    ))
}
pub(super) async fn close(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<TerminalInfo>, StatusCode> {
    let session = s.0.terminals.find(&id, &s.authorize_key(&h)?)?;
    session.close.store(true, Ordering::SeqCst);
    Ok(Json(session.snapshot()))
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(super) struct TicketInput {
    #[serde(default)]
    after: u64,
}
pub(super) async fn ticket(
    State(s): State<RemoteControl>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<TicketInput>,
) -> Result<Json<Value>, StatusCode> {
    let owner = s.authorize_key(&h)?;
    let session = s.0.terminals.find(&id, &owner)?;
    if input.after > session.state.lock().unwrap().seq {
        return Err(StatusCode::CONFLICT);
    }
    let token = random().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let expires = now() + 60;
    let mut tickets = s.0.terminals.tickets.lock().unwrap();
    tickets.retain(|_, ticket| ticket.expires > now() && ticket.id != id);
    if tickets.len() >= 64 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    tickets.insert(
        hash(&token),
        Ticket {
            id: id.clone(),
            owner,
            expires,
            after: input.after,
        },
    );
    Ok(Json(
        json!({"ticket":token,"expires_at":expires,"ws_path":format!("/remote/v1/terminals/{id}/ws?ticket={token}&after={}",input.after)}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SocketQuery {
    ticket: String,
    #[serde(default)]
    after: u64,
}
pub(super) async fn socket(
    State(s): State<RemoteControl>,
    Path(id): Path<String>,
    Query(query): Query<SocketQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let ticket =
        s.0.terminals
            .tickets
            .lock()
            .unwrap()
            .remove(&hash(&query.ticket))
            .ok_or(StatusCode::UNAUTHORIZED)?;
    if ticket.expires <= now()
        || ticket.id != id
        || ticket.after != query.after
        || !s.key_authorized(&ticket.owner)
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let session = s.0.terminals.find(&id, &ticket.owner)?;
    {
        let mut live = session.state.lock().unwrap();
        if live.connected {
            return Err(StatusCode::CONFLICT);
        }
        live.connected = true;
        live.disconnected = None;
    }
    let failed_session = session.clone();
    Ok(ws
        .max_message_size(16 * 1024)
        .max_frame_size(16 * 1024)
        .on_failed_upgrade(move |_| failed_session.disconnect())
        .on_upgrade(move |socket| connected(s, session, query.after, socket)))
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ClientMessage {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}
async fn send(socket: &mut WebSocket, value: Value) -> Result<(), ()> {
    send_frame(socket, Message::Text(value.to_string().into())).await
}
async fn send_frame(socket: &mut WebSocket, message: Message) -> Result<(), ()> {
    tokio::time::timeout(Duration::from_secs(5), socket.send(message))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
}
struct Connection(Arc<Terminal>);
impl Drop for Connection {
    fn drop(&mut self) {
        self.0.disconnect();
    }
}
async fn connected(
    host: RemoteControl,
    session: Arc<Terminal>,
    mut after: u64,
    mut socket: WebSocket,
) {
    let _connection = Connection(session.clone());
    if send(
        &mut socket,
        json!({"type":"ready","terminal":session.snapshot()}),
    )
    .await
    .is_err()
    {
        return;
    }
    let mut authorization = tokio::time::interval(Duration::from_secs(1));
    loop {
        if !host.key_authorized(&session.owner) {
            return;
        }
        // Set up the notification before snapshotting so no output wake is lost.
        let changed = session.changed.notified();
        let (gap, chunks, ended) = {
            let state = session.state.lock().unwrap();
            let first = state
                .chunks
                .front()
                .map(|(seq, _)| *seq)
                .unwrap_or(state.seq + 1);
            let gap = (after + 1 < first).then_some(first);
            let chunks: Vec<_> = state
                .chunks
                .iter()
                .filter(|(seq, _)| *seq > after)
                .cloned()
                .collect();
            let ended =
                (state.info.state != "running" && state.output_done).then(|| state.info.clone());
            (gap, chunks, ended)
        };
        if let Some(first_seq) = gap {
            if send(&mut socket, json!({"type":"gap","first_seq":first_seq}))
                .await
                .is_err()
            {
                return;
            }
            after = first_seq - 1;
        }
        for (seq, data) in chunks {
            // Slow replay must not postpone device revocation until the entire
            // retained output has been sent.
            if !host.key_authorized(&session.owner) {
                return;
            }
            if send(&mut socket, json!({"type":"output","seq":seq,"data":data}))
                .await
                .is_err()
            {
                return;
            }
            after = seq;
        }
        if let Some(terminal) = ended {
            let _ = send(&mut socket, json!({"type":"exit","terminal":terminal})).await;
            let _ = send_frame(&mut socket, Message::Close(None)).await;
            return;
        }
        tokio::select! {
            _ = changed => {},
            _ = authorization.tick() => if !host.key_authorized(&session.owner) { return; },
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => {
                    if !host.key_authorized(&session.owner) { return; }
                    let result = match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(ClientMessage::Input { data }) => session.input(data),
                        Ok(ClientMessage::Resize { cols, rows }) => session.resize(cols, rows),
                        Err(_) => Err("invalid_message"),
                    };
                    if let Err(code) = result {
                        if send(&mut socket, json!({"type":"error","code":code})).await.is_err() { return; }
                    }
                },
                Some(Ok(Message::Ping(data))) => { if send_frame(&mut socket, Message::Pong(data)).await.is_err() { return; } },
                Some(Ok(Message::Pong(_))) => {},
                Some(Ok(Message::Binary(_))) => { if send(&mut socket, json!({"type":"error","code":"invalid_message"})).await.is_err() { return; } },
                _ => return,
            },
        }
    }
}
