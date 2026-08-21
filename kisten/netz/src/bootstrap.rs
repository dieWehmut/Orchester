use std::{
    fmt,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use orchester_anwendung::{OrchesterPaths, SelfAgentHost, SessionHistory};
use orchester_verzeichnis::{standard_plugin_roots, Registry};
use serde::Serialize;

use crate::{
    agent_process::{AgentProcessSource, SystemAgentProcessSource},
    agent_status::{agent_status_response, AgentRuntimeStatusStore},
    FragmentTokenStore, FragmentTokenStoreError, ServerControl, ServerState, SessionStore,
};

#[derive(Clone)]
pub struct ServerContext {
    paths: Option<OrchesterPaths>,
    control: ServerControl,
    registry: Arc<Registry>,
    agent_status: Arc<AgentRuntimeStatusStore>,
    agent_process_source: Arc<dyn AgentProcessSource>,
    agent_process_monitor_started: Arc<AtomicBool>,
    model_host: Option<Arc<SelfAgentHost>>,
    session_history: Option<Arc<SessionHistory>>,
    sessions: Arc<SessionStore>,
    fragments: Arc<FragmentTokenStore>,
}

impl ServerContext {
    pub fn new(paths: Option<OrchesterPaths>, control: ServerControl) -> Self {
        Self::with_agent_process_source(paths, control, Arc::new(SystemAgentProcessSource))
    }

    pub fn with_agent_process_source(
        paths: Option<OrchesterPaths>,
        control: ServerControl,
        agent_process_source: Arc<dyn AgentProcessSource>,
    ) -> Self {
        let registry = Arc::new(discover_registry(paths.as_ref()));
        let agent_status = Arc::new(
            AgentRuntimeStatusStore::new(agent_status_response(&registry))
                .expect("registry status projection must validate"),
        );
        let model_host = paths.as_ref().map(SelfAgentHost::for_paths).map(Arc::new);
        let session_history = paths.as_ref().map(SessionHistory::for_paths).map(Arc::new);
        Self {
            paths,
            control,
            registry,
            agent_status,
            agent_process_source,
            agent_process_monitor_started: Arc::new(AtomicBool::new(false)),
            model_host,
            session_history,
            sessions: Arc::new(SessionStore::new(Duration::from_secs(8 * 60 * 60))),
            fragments: Arc::new(FragmentTokenStore::new(Duration::from_secs(5 * 60))),
        }
    }

    pub fn paths(&self) -> Option<&OrchesterPaths> {
        self.paths.as_ref()
    }

    pub fn control(&self) -> &ServerControl {
        &self.control
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn agent_status_store(&self) -> &AgentRuntimeStatusStore {
        &self.agent_status
    }

    pub async fn refresh_agent_processes(&self) -> Result<bool, crate::AgentRuntimeStatusError> {
        let source = Arc::clone(&self.agent_process_source);
        let snapshot = tokio::task::spawn_blocking(move || source.snapshot())
            .await
            .map_err(|_| crate::AgentRuntimeStatusError::ProcessRefreshFailed)?;
        self.agent_status
            .reconcile_external_processes(&snapshot, now_rfc3339())
    }

    /// Start the shared external-process monitor once for this server context.
    /// The task exits when the server lifecycle publishes its shutdown signal.
    pub fn start_agent_process_monitor(&self) -> bool {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return false;
        };
        if self
            .agent_process_monitor_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }

        let context = self.clone();
        runtime.spawn(async move {
            let mut shutdown = context.control.subscribe_shutdown();
            let first_tick = tokio::time::Instant::now() + Duration::from_secs(2);
            let mut ticker = tokio::time::interval_at(first_tick, Duration::from_secs(2));
            loop {
                tokio::select! {
                    biased;
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            break;
                        }
                    }
                    _ = ticker.tick() => {
                        let _ = context.refresh_agent_processes().await;
                    }
                }
            }
        });
        true
    }

    pub fn model_host(&self) -> Option<&SelfAgentHost> {
        self.model_host.as_deref()
    }

    pub fn session_history(&self) -> Option<&SessionHistory> {
        self.session_history.as_deref()
    }

    pub(crate) fn sessions(&self) -> &SessionStore {
        &self.sessions
    }

    pub fn provision_fragment_token(&self, token: &str) -> Result<(), FragmentTokenStoreError> {
        self.fragments.register(token)
    }

    pub(crate) fn fragments(&self) -> &FragmentTokenStore {
        &self.fragments
    }
}

impl fmt::Debug for ServerContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServerContext")
            .field("workspace_selected", &self.paths.is_some())
            .field("server_state", &self.control.state())
            .field("agent_count", &self.registry.len())
            .finish_non_exhaustive()
    }
}

fn discover_registry(paths: Option<&OrchesterPaths>) -> Registry {
    let Some(paths) = paths else {
        let mut registry = Registry::new();
        registry.register_builtins();
        return registry;
    };

    match standard_plugin_roots(paths.home(), paths.workspace()) {
        Ok(plugin_roots) => {
            Registry::discover_with_plugin_roots(paths.manifest_dir(), plugin_roots)
        }
        Err(_) => Registry::discover(paths.manifest_dir()),
    }
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BootstrapWorkspaceDto {
    pub selected: bool,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BootstrapDto {
    pub schema_version: u8,
    pub service_version: String,
    pub server_state: ServerState,
    pub workspace: BootstrapWorkspaceDto,
}

pub fn bootstrap_response(context: &ServerContext) -> BootstrapDto {
    let workspace = context
        .paths()
        .map(|paths| BootstrapWorkspaceDto {
            selected: true,
            name: safe_basename(paths.workspace()),
        })
        .unwrap_or(BootstrapWorkspaceDto {
            selected: false,
            name: None,
        });

    BootstrapDto {
        schema_version: 1,
        service_version: env!("CARGO_PKG_VERSION").to_owned(),
        server_state: context.control().state(),
        workspace,
    }
}

fn safe_basename(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if name.is_empty() || name.chars().any(char::is_control) {
        return None;
    }
    Some(name.to_owned())
}
