use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::descriptor::valid_name;
use super::{LoadedAgentPlugin, PluginInfo, load_agent_plugin};

const MAX_PLUGINS_PER_ROOT: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PluginRootError {
    #[error("managed plugin home must be an absolute path")]
    ManagedHomeNotAbsolute,
    #[error("project plugin directory must be an absolute path")]
    ProjectDirectoryNotAbsolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginOrigin {
    Managed,
    Project,
}

pub struct PluginRoot {
    path: PathBuf,
    origin: PluginOrigin,
}

impl PluginRoot {
    pub fn managed(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            origin: PluginOrigin::Managed,
        }
    }

    pub fn project(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            origin: PluginOrigin::Project,
        }
    }

    pub fn origin(&self) -> PluginOrigin {
        self.origin
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl fmt::Debug for PluginRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PluginRoot")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

pub fn standard_plugin_roots(
    managed_home: impl AsRef<Path>,
    project_directory: impl AsRef<Path>,
) -> Result<[PluginRoot; 2], PluginRootError> {
    let managed_home = managed_home.as_ref();
    if !managed_home.is_absolute() {
        return Err(PluginRootError::ManagedHomeNotAbsolute);
    }
    let project_directory = project_directory.as_ref();
    if !project_directory.is_absolute() {
        return Err(PluginRootError::ProjectDirectoryNotAbsolute);
    }
    Ok([
        PluginRoot::managed(
            managed_home
                .join("plugins")
                .join("npm")
                .join("node_modules")
                .join("@orchester"),
        ),
        PluginRoot::project(project_directory.join("node_modules").join("@orchester")),
    ])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredPlugin {
    info: PluginInfo,
    origin: PluginOrigin,
}

impl RegisteredPlugin {
    pub(crate) fn new(info: PluginInfo, origin: PluginOrigin) -> Self {
        Self { info, origin }
    }

    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    pub fn origin(&self) -> PluginOrigin {
        self.origin
    }
}

pub(crate) fn load_root(root: &PluginRoot) -> Vec<LoadedAgentPlugin> {
    let Ok(metadata) = fs::symlink_metadata(root.path()) else {
        return Vec::new();
    };
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(root.path()) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for (seen, entry) in entries.enumerate() {
        if seen == MAX_PLUGINS_PER_ROOT {
            return Vec::new();
        }
        let Ok(entry) = entry else {
            return Vec::new();
        };
        let name = entry.file_name();
        let Some(name_text) = name.to_str() else {
            continue;
        };
        if valid_name(name_text) {
            names.push(name);
        }
    }
    names.sort();
    names
        .into_iter()
        .filter_map(|name| load_agent_plugin(root.path().join(name)).ok())
        .collect()
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
