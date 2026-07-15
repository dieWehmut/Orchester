mod descriptor;
mod discovery;
mod package;

use std::fmt;
use std::path::Path;

use orchester_vertrag::{AgentAdapter, ManifestAdapter};
use thiserror::Error;

use descriptor::ValidatedDescriptor;

pub(crate) use discovery::load_root;
pub use discovery::{PluginOrigin, PluginRoot, RegisteredPlugin};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    name: String,
    display_name: String,
    package_name: String,
    version: String,
}

impl PluginInfo {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

pub struct LoadedAgentPlugin {
    info: PluginInfo,
    adapter: ManifestAdapter,
}

impl LoadedAgentPlugin {
    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    pub fn adapter(&self) -> &ManifestAdapter {
        &self.adapter
    }

    pub fn into_adapter(self) -> ManifestAdapter {
        self.adapter
    }

    pub(crate) fn into_parts(self) -> (PluginInfo, ManifestAdapter) {
        (self.info, self.adapter)
    }
}

impl fmt::Debug for LoadedAgentPlugin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedAgentPlugin")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PluginError {
    #[error("agent plugin package is unavailable")]
    Unavailable,
    #[error("agent plugin package has invalid members")]
    InvalidPackage,
    #[error("agent plugin descriptor is invalid")]
    InvalidDescriptor,
    #[error("agent plugin adapter manifest is invalid")]
    InvalidManifest,
    #[error("agent plugin package exceeds a size limit")]
    LimitExceeded,
}

pub fn load_agent_plugin(package_root: impl AsRef<Path>) -> Result<LoadedAgentPlugin, PluginError> {
    let package = package::read(package_root.as_ref())?;
    let descriptor = descriptor::validate(&package.package_json, &package.descriptor_json)?;
    let adapter = ManifestAdapter::from_toml(&package.adapter_manifest)
        .map_err(|_| PluginError::InvalidManifest)?;
    validate_adapter(&descriptor, &adapter)?;

    Ok(LoadedAgentPlugin {
        info: PluginInfo {
            name: descriptor.name,
            display_name: descriptor.display_name,
            package_name: descriptor.package_name,
            version: descriptor.version,
        },
        adapter,
    })
}

fn validate_adapter(
    descriptor: &ValidatedDescriptor,
    adapter: &ManifestAdapter,
) -> Result<(), PluginError> {
    if adapter.name() != descriptor.name
        || adapter.native_command() != Some(descriptor.command.as_str())
    {
        return Err(PluginError::InvalidManifest);
    }
    Ok(())
}
