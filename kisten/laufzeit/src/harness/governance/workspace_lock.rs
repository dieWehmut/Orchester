use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};

use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

use super::{GuardError, PathResolver, ResolvedPath, WorkspaceGuard};

/// Process-local lock registry keyed by the durable workspace identity.
#[derive(Clone, Default)]
pub struct WorkspaceLocks {
    inner: Arc<WorkspaceLocksInner>,
}

#[derive(Default)]
struct WorkspaceLocksInner {
    locks: Mutex<HashMap<String, Weak<RwLock<()>>>>,
}

impl WorkspaceLocks {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn read(&self, identity: &str) -> OwnedRwLockReadGuard<()> {
        self.lock_for(identity).read_owned().await
    }

    pub async fn mutate(&self, identity: &str) -> OwnedRwLockWriteGuard<()> {
        self.lock_for(identity).write_owned().await
    }

    /// Acquire the workspace mutation lock before resolving the target path.
    pub async fn resolve_mutation<R: PathResolver>(
        &self,
        identity: &str,
        workspace: &WorkspaceGuard<R>,
        requested: &Path,
    ) -> Result<MutationLease, GuardError> {
        let guard = self.mutate(identity).await;
        let resolved = workspace.resolve_write(requested)?;
        Ok(MutationLease {
            _guard: guard,
            resolved,
        })
    }

    fn lock_for(&self, identity: &str) -> Arc<RwLock<()>> {
        let mut locks = self
            .inner
            .locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = locks.get(identity).and_then(Weak::upgrade) {
            return existing;
        }
        let lock = Arc::new(RwLock::new(()));
        locks.insert(identity.to_owned(), Arc::downgrade(&lock));
        lock
    }
}

/// Holds the exclusive workspace lock for as long as its resolved path is used.
#[derive(Debug)]
pub struct MutationLease {
    _guard: OwnedRwLockWriteGuard<()>,
    resolved: ResolvedPath,
}

impl MutationLease {
    pub fn resolved(&self) -> &ResolvedPath {
        &self.resolved
    }
}
