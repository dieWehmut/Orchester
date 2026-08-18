use std::{path::Path, sync::Arc, time::Duration};

use orchester_anwendung::OrchesterPaths;
use serde::Serialize;

use crate::{ServerControl, ServerState, SessionStore};

#[derive(Debug, Clone)]
pub struct ServerContext {
    paths: Option<OrchesterPaths>,
    control: ServerControl,
    sessions: Arc<SessionStore>,
}

impl ServerContext {
    pub fn new(paths: Option<OrchesterPaths>, control: ServerControl) -> Self {
        Self {
            paths,
            control,
            sessions: Arc::new(SessionStore::new(Duration::from_secs(8 * 60 * 60))),
        }
    }

    pub fn paths(&self) -> Option<&OrchesterPaths> {
        self.paths.as_ref()
    }

    pub fn control(&self) -> &ServerControl {
        &self.control
    }

    pub(crate) fn sessions(&self) -> &SessionStore {
        &self.sessions
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
