use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};

use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};

use super::{GuardError, ResolvedPath, WorkspaceGuard, WorkspaceIdentity};

/// Process-local lock registry keyed by the durable workspace identity.
#[derive(Clone, Default)]
pub struct WorkspaceLocks {
    inner: Arc<WorkspaceLocksInner>,
}

#[derive(Default)]
struct WorkspaceLocksInner {
    locks: Mutex<HashMap<WorkspaceIdentity, Weak<RwLock<()>>>>,
}

impl WorkspaceLocks {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn read(&self, workspace: &WorkspaceGuard) -> OwnedRwLockReadGuard<()> {
        self.lock_for(workspace.identity()).read_owned().await
    }

    pub async fn mutate(&self, workspace: &WorkspaceGuard) -> OwnedRwLockWriteGuard<()> {
        self.lock_for(workspace.identity()).write_owned().await
    }

    /// Acquire the workspace mutation lock before resolving the target path.
    pub async fn resolve_mutation(
        &self,
        workspace: &WorkspaceGuard,
        requested: &Path,
    ) -> Result<MutationLease, GuardError> {
        let guard = self.mutate(workspace).await;
        let resolved = workspace.resolve_write(requested)?;
        Ok(MutationLease {
            _guard: guard,
            resolved,
        })
    }

    fn lock_for(&self, identity: &WorkspaceIdentity) -> Arc<RwLock<()>> {
        let mut locks = self
            .inner
            .locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = locks.get(identity).and_then(Weak::upgrade) {
            return existing;
        }
        let lock = Arc::new(RwLock::new(()));
        locks.insert(identity.clone(), Arc::downgrade(&lock));
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
