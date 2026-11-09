//! Blocking PTY IO stays off Tokio workers. Each session owns one reader, one
//! bounded input worker and one supervisor that can kill even blocked writers.
use super::*;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

const REPLAY_BYTES: usize = 256 * 1024;
const REPLAY_CHUNKS: usize = 512;
const DISCONNECT_TTL: Duration = Duration::from_secs(5 * 60);
const INPUT_IDLE_TTL: Duration = Duration::from_secs(30 * 60);

pub(super) struct Terminal {
    pub owner: String,
    pub state: Mutex<Live>,
    pub changed: tokio::sync::Notify,
    pub close: AtomicBool,
    pub workers_done: AtomicBool,
    input: SyncSender<Input>,
    resize: SyncSender<(u16, u16)>,
}
enum Input {
    User(String),
    #[cfg(windows)]
    InitialCursor,
}
pub(super) struct Live {
    pub info: TerminalInfo,
    pub chunks: std::collections::VecDeque<(u64, String)>,
    pub seq: u64,
    pub output_done: bool,
    bytes: usize,
    pub connected: bool,
    pub disconnected: Option<Instant>,
    last_input: Instant,
}

fn shell_command(directory: &std::path::Path) -> (String, CommandBuilder) {
    #[cfg(windows)]
    {
        // cmd cannot use UNC working directories; PowerShell supports them.
        if directory.to_string_lossy().starts_with("\\\\") {
            let shell = std::path::PathBuf::from(
                std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
            )
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            let mut command = CommandBuilder::new(&shell);
            command.args(["-NoLogo", "-NoProfile"]);
            return (shell.to_string_lossy().into_owned(), command);
        }
        let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        let mut command = CommandBuilder::new(&shell);
        command.args(["/D", "/Q"]); // Disable registry AutoRun commands.
        (shell.to_string_lossy().into_owned(), command)
    }
    #[cfg(not(windows))]
    {
        let _ = directory;
        let shell = if std::path::Path::new("/bin/bash").is_file() {
            "/bin/bash"
        } else {
            "/bin/sh"
        };
        let mut command = CommandBuilder::new(shell);
        if shell == "/bin/bash" {
            command.args(["--noprofile", "--norc"]);
        }
        command.arg("-i");
        // Avoid startup hooks inherited from the host environment.
        command.env_remove("ENV");
        command.env_remove("BASH_ENV");
        (shell.to_owned(), command)
    }
}

fn command_directory(path: &std::path::Path) -> PathBuf {
    #[cfg(windows)]
    {
        // canonicalize uses a verbatim path prefix. cmd treats it as UNC and
        // silently changes to C:\\Windows, so pass the equivalent normal path.
        let path = path.to_string_lossy();
        if let Some(unc) = path.strip_prefix("\\\\?\\UNC\\") {
            return PathBuf::from(format!("\\\\{unc}"));
        }
        if let Some(drive) = path.strip_prefix("\\\\?\\") {
            return PathBuf::from(drive);
        }
    }
    path.to_path_buf()
}

struct PtyOwner {
    child: Box<dyn Child + Send + Sync>,
    master: Box<dyn MasterPty + Send>,
    tree: orchester_vertrag::process::ProcessTree,
    pid: u32,
    terminated: bool,
}
impl PtyOwner {
    fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        #[cfg(unix)]
        let candidates = {
            let system = sysinfo::System::new_all();
            system
                .processes()
                .keys()
                .map(|pid| pid.as_u32())
                .collect::<Vec<_>>()
        };
        #[cfg(not(unix))]
        let candidates = Vec::new();
        self.tree.terminate_pty(self.pid, &candidates);
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
        }
    }
}
impl Drop for PtyOwner {
    fn drop(&mut self) {
        self.terminate();
        // Dropping a native child handle alone does not reap Unix children.
        let _ = self.child.wait();
    }
}

impl Terminal {
    pub fn spawn(
        owner: String,
        project: Project,
        cols: u16,
        rows: u16,
    ) -> std::io::Result<Arc<Self>> {
        let id = random()?;
        let directory = command_directory(&project.path);
        let (shell, mut command) = shell_command(&directory);
        command.cwd(directory);
        command.env("TERM", "xterm-256color");
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(std::io::Error::other)?;
        let tree = orchester_vertrag::process::ProcessTree::new()?;
        // Acquire fallible IO handles before creating a child to avoid leaking
        // a shell when descriptor duplication or writer setup fails.
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(std::io::Error::other)?;
        let writer = pair.master.take_writer().map_err(std::io::Error::other)?;
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(std::io::Error::other)?;
        let Some(pid) = child.process_id() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::other("PTY child has no process ID"));
        };
        drop(pair.slave);
        let resources = PtyOwner {
            child,
            master: pair.master,
            tree,
            pid,
            terminated: false,
        };
        let (input, input_rx) = mpsc::sync_channel(16);
        let (resize, resize_rx) = mpsc::sync_channel(4);
        let session = Arc::new(Self {
            owner,
            state: Mutex::new(Live {
                info: TerminalInfo {
                    id,
                    project_id: project.id,
                    workspace: project.path,
                    shell,
                    cols,
                    rows,
                    state: "running".into(),
                    created_at: now(),
                    exit_code: None,
                },
                chunks: Default::default(),
                seq: 0,
                output_done: false,
                bytes: 0,
                connected: false,
                disconnected: Some(Instant::now()),
                last_input: Instant::now(),
            }),
            input,
            resize,
            changed: tokio::sync::Notify::new(),
            close: AtomicBool::new(false),
            workers_done: AtomicBool::new(false),
        });
        let read_session = session.clone();
        let reader_thread = std::thread::spawn(move || read_output(read_session, reader));
        let write_session = session.clone();
        let writer_thread =
            std::thread::spawn(move || write_input(write_session, writer, input_rx));
        // ConPTY may request its initial cursor even while a failed attachment
        // is closing. Start both IO workers before any child cleanup can occur.
        if let Err(error) = resources.tree.attach_pid(pid) {
            session.failed();
            drop(resources);
            let _ = reader_thread.join();
            let _ = writer_thread.join();
            return Err(error);
        }
        let supervise_session = session.clone();
        std::thread::spawn(move || {
            supervise(supervise_session.clone(), resources, resize_rx);
            // Killing the entire session closes slave descriptors and unblocks IO.
            let _ = reader_thread.join();
            let _ = writer_thread.join();
            supervise_session.workers_done.store(true, Ordering::SeqCst);
            supervise_session.changed.notify_one();
        });
        Ok(session)
    }
    pub fn snapshot(&self) -> TerminalInfo {
        self.state.lock().unwrap().info.clone()
    }
    pub fn disconnect(&self) {
        let mut state = self.state.lock().unwrap();
        state.connected = false;
        state.disconnected = Some(Instant::now());
    }
    pub fn input(&self, data: String) -> Result<(), &'static str> {
        if data.len() > 8192 {
            return Err("invalid_message");
        }
        if self.close.load(Ordering::SeqCst) || self.snapshot().state != "running" {
            return Err("terminal_closed");
        }
        self.input
            .try_send(Input::User(data))
            .map_err(|_| "input_busy")?;
        self.state.lock().unwrap().last_input = Instant::now();
        Ok(())
    }
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), &'static str> {
        if !valid_size(cols, rows) {
            return Err("invalid_message");
        }
        if self.close.load(Ordering::SeqCst) || self.snapshot().state != "running" {
            return Err("terminal_closed");
        }
        self.resize.try_send((cols, rows)).map_err(|_| "input_busy")
    }
    fn output(&self, data: String) {
        if data.is_empty() {
            return;
        }
        let mut state = self.state.lock().unwrap();
        state.seq += 1;
        let seq = state.seq;
        state.bytes += data.len();
        state.chunks.push_back((seq, data));
        while state.bytes > REPLAY_BYTES || state.chunks.len() > REPLAY_CHUNKS {
            if let Some((_, chunk)) = state.chunks.pop_front() {
                state.bytes -= chunk.len();
            }
        }
        drop(state);
        self.changed.notify_one();
    }
    fn failed(&self) {
        let mut state = self.state.lock().unwrap();
        if state.info.state == "running" {
            state.info.state = "failed".into();
        }
        drop(state);
        self.close.store(true, Ordering::SeqCst);
        self.changed.notify_one();
    }
}

fn read_output(session: Arc<Terminal>, mut reader: Box<dyn Read + Send>) {
    let mut bytes = [0u8; 4096];
    let mut pending = Vec::new();
    #[cfg(windows)]
    let mut initial_cursor = true;
    loop {
        match reader.read(&mut bytes) {
            Ok(0) => break,
            Ok(count) => {
                pending.extend_from_slice(&bytes[..count]);
                #[cfg(windows)]
                if initial_cursor {
                    // portable-pty requests an inherited cursor for ConPTY.
                    // This is a new blank screen, so initialize it ourselves
                    // without waiting for a browser (or replaying the query).
                    if pending.len() < 4 {
                        continue;
                    }
                    if pending.starts_with(b"\x1b[6n") {
                        // This protocol response must still be accepted during
                        // close: ClosePseudoConsole waits for cursor startup.
                        let _ = session.input.send(Input::InitialCursor);
                        pending.drain(..4);
                    }
                    initial_cursor = false;
                }
                session.output(decode_pending(&mut pending, false));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            // Unix PTYs report EIO when their final slave closes.
            Err(_) => break,
        }
    }
    session.output(decode_pending(&mut pending, true));
    session.state.lock().unwrap().output_done = true;
    session.changed.notify_one();
}
fn decode_pending(pending: &mut Vec<u8>, final_chunk: bool) -> String {
    let mut output = String::new();
    let mut consumed = 0;
    while consumed < pending.len() {
        match std::str::from_utf8(&pending[consumed..]) {
            Ok(text) => {
                output.push_str(text);
                consumed = pending.len();
            }
            Err(error) => {
                let valid = consumed + error.valid_up_to();
                output.push_str(std::str::from_utf8(&pending[consumed..valid]).unwrap());
                consumed = valid;
                if let Some(length) = error.error_len() {
                    output.push('\u{fffd}');
                    consumed += length;
                } else {
                    if final_chunk {
                        output.push('\u{fffd}');
                        consumed = pending.len();
                    }
                    break;
                }
            }
        }
    }
    pending.drain(..consumed);
    output
}
fn write_input(session: Arc<Terminal>, mut writer: Box<dyn Write + Send>, input: Receiver<Input>) {
    loop {
        if session.close.load(Ordering::SeqCst) && session.state.lock().unwrap().output_done {
            break;
        }
        match input.recv_timeout(Duration::from_millis(50)) {
            Ok(message) => {
                let data = match message {
                    Input::User(data) if !session.close.load(Ordering::SeqCst) => data,
                    Input::User(_) => continue,
                    #[cfg(windows)]
                    Input::InitialCursor => "\x1b[1;1R".into(),
                };
                if writer
                    .write_all(data.as_bytes())
                    .and_then(|_| writer.flush())
                    .is_err()
                {
                    session.failed();
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
}
fn supervise(session: Arc<Terminal>, mut pty: PtyOwner, resize: Receiver<(u16, u16)>) {
    loop {
        let expired = {
            let state = session.state.lock().unwrap();
            state
                .disconnected
                .is_some_and(|at| at.elapsed() >= DISCONNECT_TTL)
                || state.last_input.elapsed() >= INPUT_IDLE_TTL
        };
        if session.close.load(Ordering::SeqCst) || expired {
            pty.terminate();
            let status = pty.child.wait().ok();
            let mut state = session.state.lock().unwrap();
            if state.info.state == "running" {
                state.info.state = "closed".into();
            }
            state.info.exit_code = status.map(|status| status.exit_code());
            break;
        }
        while let Ok((cols, rows)) = resize.try_recv() {
            if pty
                .master
                .resize(PtySize {
                    cols,
                    rows,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .is_err()
            {
                session.failed();
                break;
            }
            let mut state = session.state.lock().unwrap();
            state.info.cols = cols;
            state.info.rows = rows;
        }
        match pty.child.try_wait() {
            Ok(Some(status)) => {
                // Clean up surviving background jobs even after the shell exits.
                pty.terminate();
                let mut state = session.state.lock().unwrap();
                state.info.state = "exited".into();
                state.info.exit_code = Some(status.exit_code());
                break;
            }
            Ok(None) => {}
            Err(_) => {
                session.failed();
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    session.close.store(true, Ordering::SeqCst);
    // ConPTY closes here while the separate reader continues draining its pipe.
    drop(pty);
    session.changed.notify_one();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiry_and_manager_drop_release_shell_and_io_workers() {
        for cause in ["disconnect", "idle", "revoke", "manager_drop"] {
            let root = tempfile::tempdir().unwrap();
            let session = Terminal::spawn(
                "fixture".into(),
                Project::from_path(root.path().canonicalize().unwrap()),
                80,
                24,
            )
            .unwrap();
            let mut retained_manager = None;
            match cause {
                "disconnect" => {
                    session.state.lock().unwrap().disconnected =
                        Some(Instant::now() - DISCONNECT_TTL)
                }
                "idle" => {
                    let mut live = session.state.lock().unwrap();
                    live.disconnected = None;
                    live.last_input = Instant::now() - INPUT_IDLE_TTL;
                }
                _ => {
                    let manager = Terminals::default();
                    manager
                        .sessions
                        .lock()
                        .unwrap()
                        .insert(session.snapshot().id.clone(), session.clone());
                    if cause == "revoke" {
                        manager.revoke("fixture");
                        retained_manager = Some(manager);
                    } else {
                        drop(manager);
                    }
                }
            }
            let started = Instant::now();
            loop {
                let ended = {
                    let live = session.state.lock().unwrap();
                    live.info.state != "running" && live.output_done
                };
                if ended {
                    drop(retained_manager.take());
                    if session.workers_done.load(Ordering::SeqCst)
                        && Arc::strong_count(&session) == 1
                    {
                        break;
                    }
                }
                assert!(
                    started.elapsed() < Duration::from_secs(5),
                    "{cause} leaked PTY workers"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(session.snapshot().state, "closed");
        }
    }
    #[cfg(windows)]
    #[test]
    fn immediate_conpty_close_repeatedly_drains_startup_and_io() {
        let root = tempfile::tempdir().unwrap();
        for attempt in 0..24 {
            let session = Terminal::spawn(
                "fixture".into(),
                Project::from_path(root.path().canonicalize().unwrap()),
                80,
                24,
            )
            .unwrap();
            session.close.store(true, Ordering::SeqCst);
            let started = Instant::now();
            while !session.workers_done.load(Ordering::SeqCst) {
                assert!(
                    started.elapsed() < Duration::from_secs(5),
                    "ConPTY startup close {attempt} leaked IO workers"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(session.snapshot().state, "closed");
        }
    }
    #[test]
    fn utf8_preserves_cjk_across_every_byte_boundary_and_invalid_bytes() {
        let expected = "你好终端🌏";
        let mut pending = Vec::new();
        let mut actual = String::new();
        for byte in expected.as_bytes() {
            pending.push(*byte);
            actual.push_str(&decode_pending(&mut pending, false));
        }
        actual.push_str(&decode_pending(&mut pending, true));
        assert_eq!(actual, expected);
        pending.extend_from_slice(&[0xff, 0xe4]);
        assert_eq!(decode_pending(&mut pending, false), "\u{fffd}");
        pending.extend_from_slice(&[0xbd, 0xa0]);
        assert_eq!(decode_pending(&mut pending, true), "你");
    }
    #[test]
    fn replay_memory_and_chunk_count_are_bounded() {
        let (input, _) = mpsc::sync_channel(1);
        let (resize, _) = mpsc::sync_channel(1);
        let session = Terminal {
            owner: String::new(),
            input,
            resize,
            close: AtomicBool::new(false),
            workers_done: AtomicBool::new(true),
            changed: tokio::sync::Notify::new(),
            state: Mutex::new(Live {
                info: TerminalInfo {
                    id: "test".into(),
                    project_id: "test".into(),
                    workspace: "/".into(),
                    shell: "test".into(),
                    cols: 80,
                    rows: 24,
                    state: "running".into(),
                    created_at: 0,
                    exit_code: None,
                },
                chunks: Default::default(),
                seq: 0,
                output_done: false,
                bytes: 0,
                connected: false,
                disconnected: None,
                last_input: Instant::now(),
            }),
        };
        for _ in 0..1000 {
            session.output("x".repeat(4096));
        }
        let state = session.state.lock().unwrap();
        assert_eq!(state.seq, 1000);
        assert!(state.bytes <= REPLAY_BYTES);
        assert!(state.chunks.len() <= REPLAY_CHUNKS);
        assert!(state.chunks.front().unwrap().0 > 1);
    }
}
