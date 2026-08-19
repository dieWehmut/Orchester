use std::{fmt, path::Path, sync::Arc, time::Duration};

use orchester_anwendung::OrchesterPaths;
use orchester_verzeichnis::{standard_plugin_roots, Registry};
use serde::Serialize;

use crate::{
    FragmentTokenStore, FragmentTokenStoreError, ServerControl, ServerState, SessionStore,
};

#[derive(Clone)]
pub struct ServerContext {
    paths: Option<OrchesterPaths>,
    control: ServerControl,
    registry: Arc<Registry>,
    sessions: Arc<SessionStore>,
    fragments: Arc<FragmentTokenStore>,
}

impl ServerContext {
    pub fn new(paths: Option<OrchesterPaths>, control: ServerControl) -> Self {
        let registry = Arc::new(discover_registry(paths.as_ref()));
        Self {
            paths,
            control,
            registry,
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
