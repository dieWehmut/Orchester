//! Adapter discovery and lookup.
//!
//! The [`Registry`] is Orchester's index of runnable agents. It registers the
//! built-ins (the scripted `mock` plus the three embedded manifest agents), then
//! overlays any `*.toml` manifests found on disk under a manifest directory.
//! Disk manifests win on a name collision, so a user can override a built-in
//! (e.g. tweak claude's flags) without recompiling.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use orchester_adapter::{builtin, MockAdapter, BUILTIN_MANIFESTS};
use orchester_protokoll::Capability;
use orchester_vertrag::{AdapterAvailability, AdapterError, AgentAdapter, ManifestAdapter};

mod plugin;

pub use plugin::{
    load_agent_plugin, LoadedAgentPlugin, PluginError, PluginInfo, PluginOrigin, PluginRoot,
    RegisteredPlugin,
};

/// An index of adapters keyed by name.
///
/// `BTreeMap` keeps `list()` output stable (alphabetical), which makes the CLI
/// `list` command and its tests deterministic.
pub struct Registry {
    adapters: BTreeMap<String, Arc<dyn AgentAdapter>>,
    plugins: BTreeMap<String, RegisteredPlugin>,
}

impl Registry {
    /// An empty registry. Prefer [`Registry::discover`] for normal use.
    pub fn new() -> Self {
        Self {
            adapters: BTreeMap::new(),
            plugins: BTreeMap::new(),
        }
    }

    /// Register the built-ins, then overlay disk manifests from `manifest_dir`.
    ///
    /// A missing or unreadable directory is not an error — the built-ins alone
    /// form a working registry. Individual malformed manifests are logged and
    /// skipped rather than aborting discovery.
    pub fn discover(manifest_dir: impl AsRef<Path>) -> Self {
        Self::discover_with_plugin_roots(manifest_dir, std::iter::empty())
    }

    pub fn discover_with_plugin_roots(
        manifest_dir: impl AsRef<Path>,
        plugin_roots: impl IntoIterator<Item = PluginRoot>,
    ) -> Self {
        let mut registry = Self::new();
        registry.register_builtins();
        registry.load_dir(manifest_dir.as_ref());
        for root in plugin_roots {
            registry.load_plugin_root(&root);
        }
        registry
    }

    /// Register only the compiled-in adapters (mock + embedded manifests).
    /// Useful when no on-disk manifest directory is available.
    pub fn register_builtins(&mut self) {
        self.insert(Arc::new(MockAdapter::new()));
        for (name, _) in BUILTIN_MANIFESTS {
            match builtin(name) {
                Ok(Some(adapter)) => self.insert(Arc::new(adapter)),
                // Embedded manifests are validated by adapter's own tests, so a
                // failure here is a build-time bug, not a runtime condition.
                Ok(None) => {}
                Err(e) => tracing::warn!(agent = name, error = %e, "embedded manifest failed"),
            }
        }
    }

    /// Load every `*.toml` in `dir` as a [`ManifestAdapter`], overriding any
    /// existing adapter of the same name.
    fn load_dir(&mut self, dir: &Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return, // no manifest dir → built-ins only
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match load_manifest(&path) {
                Ok(adapter) => self.insert(Arc::new(adapter)),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping bad manifest")
                }
            }
        }
    }

    fn insert(&mut self, adapter: Arc<dyn AgentAdapter>) {
        self.adapters.insert(adapter.name().to_string(), adapter);
    }

    fn load_plugin_root(&mut self, root: &PluginRoot) {
        for loaded in plugin::load_root(root) {
            let (info, adapter) = loaded.into_parts();
            let name = info.name().to_owned();
            self.insert(Arc::new(adapter));
            self.plugins
                .insert(name, RegisteredPlugin::new(info, root.origin()));
        }
    }

    /// Look up an adapter by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn AgentAdapter>> {
        self.adapters.get(name).cloned()
    }

    /// Native interactive command for an adapter, when the adapter wraps a CLI.
    pub fn native_command(&self, name: &str) -> Option<String> {
        self.adapters
            .get(name)
            .and_then(|adapter| adapter.native_command())
            .map(str::to_string)
    }

    /// The capabilities of every registered adapter, alphabetical by name.
    pub fn list(&self) -> Vec<Capability> {
        self.adapters.values().map(|a| a.capabilities()).collect()
    }

    /// Availability checks for every registered adapter, alphabetical by name.
    pub fn availability(&self) -> Vec<AdapterAvailability> {
        self.adapters.values().map(|a| a.availability()).collect()
    }

    pub fn plugins(&self) -> Vec<RegisteredPlugin> {
        self.plugins.values().cloned().collect()
    }

    /// Number of registered adapters.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Whether the registry has no adapters.
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// Read and parse one manifest file into a [`ManifestAdapter`].
fn load_manifest(path: &Path) -> Result<ManifestAdapter, AdapterError> {
    let toml_str = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
    ManifestAdapter::from_toml(&toml_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_registered() {
        let mut registry = Registry::new();
        registry.register_builtins();
        // mock + claude + codex + opencode
        assert_eq!(registry.len(), 4);
        assert!(registry.get("mock").is_some());
        assert!(registry.get("claude").is_some());
        assert!(registry.get("codex").is_some());
        assert!(registry.get("opencode").is_some());
        assert!(registry.get("nope").is_none());
    }

    #[test]
    fn list_is_alphabetical() {
        let mut registry = Registry::new();
        registry.register_builtins();
        let names: Vec<_> = registry.list().into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["claude", "codex", "mock", "opencode"]);
    }

    #[test]
    fn missing_dir_falls_back_to_builtins() {
        let registry = Registry::discover("this/dir/does/not/exist");
        assert_eq!(registry.len(), 4);
    }

    #[test]
    fn availability_is_alphabetical_and_includes_mock() {
        let mut registry = Registry::new();
        registry.register_builtins();

        let checks = registry.availability();
        let names: Vec<_> = checks.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["claude", "codex", "mock", "opencode"]);
        let mock = checks.iter().find(|c| c.name == "mock").unwrap();
        assert!(!mock.is_missing());
    }
}
