use std::fs;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use orchester_laufzeit::harness::governance::{
    FilesystemResolver, GuardErrorKind, PathResolver, WorkspaceGuard, WorkspaceLocks,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempWorkspace {
    root: PathBuf,
    outside: PathBuf,
}

impl TempWorkspace {
    fn new() -> Self {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("orchester-path-guard-{}-{id}", std::process::id()));
        let root = base.join("workspace");
        let outside = base.join("outside");
        fs::create_dir_all(root.join("src")).expect("create workspace");
        fs::create_dir_all(&outside).expect("create outside directory");
        Self { root, outside }
    }

    fn guard(&self) -> WorkspaceGuard {
        WorkspaceGuard::new(&self.root).expect("workspace guard")
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        if let Some(base) = self.root.parent() {
            let _ = fs::remove_dir_all(base);
        }
    }
}

#[test]
fn rejects_parent_escape_absolute_outside_empty_and_nul_paths() {
    let ws = TempWorkspace::new();
    let guard = ws.guard();

    assert_eq!(
        guard
            .resolve_read(Path::new("../secret"))
            .unwrap_err()
            .kind(),
        GuardErrorKind::Outside
    );
    assert_eq!(
        guard
            .resolve_write(&ws.outside.join("new.rs"))
            .unwrap_err()
            .kind(),
        GuardErrorKind::Outside
    );
    assert_eq!(
        guard.resolve_write(Path::new("")).unwrap_err().kind(),
        GuardErrorKind::InvalidPath
    );
    assert_eq!(
        guard
            .resolve_write(Path::new("src/bad\0name"))
            .unwrap_err()
            .kind(),
        GuardErrorKind::InvalidPath
    );
}

#[test]
fn permits_existing_reads_and_new_files_below_real_workspace_parents() {
    let ws = TempWorkspace::new();
    fs::write(ws.root.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();
    let guard = ws.guard();

    let existing = guard.resolve_read(Path::new("src/lib.rs")).unwrap();
    assert_eq!(
        existing.final_path,
        fs::canonicalize(ws.root.join("src/lib.rs")).unwrap()
    );

    let future = guard.resolve_write(Path::new("src/nested/new.rs")).unwrap();
    assert!(future.final_path.starts_with(guard.root()));
    assert_eq!(
        future.canonical_parent,
        fs::canonicalize(ws.root.join("src")).unwrap()
    );
}

#[test]
fn accepts_an_absolute_path_that_is_inside_the_workspace() {
    let ws = TempWorkspace::new();
    fs::write(ws.root.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();
    let guard = ws.guard();

    let absolute = ws.root.join("src/lib.rs");
    let resolved = guard.resolve_read(&absolute).unwrap();
    assert_eq!(resolved.final_path, fs::canonicalize(absolute).unwrap());
}

#[test]
fn existing_directory_reports_its_parent_not_itself() {
    let ws = TempWorkspace::new();
    let guard = ws.guard();

    let directory = guard.resolve_read(Path::new("src")).unwrap();
    assert_eq!(
        directory.canonical_parent,
        fs::canonicalize(&ws.root).unwrap()
    );
}

#[test]
fn rejects_missing_reads_non_directory_components_and_protected_state() {
    let ws = TempWorkspace::new();
    fs::write(ws.root.join("plain-file"), "content").unwrap();
    let guard = ws.guard();

    assert_eq!(
        guard.resolve_read(Path::new("missing")).unwrap_err().kind(),
        GuardErrorKind::NotFound
    );
    assert_eq!(
        guard
            .resolve_write(Path::new("plain-file/child"))
            .unwrap_err()
            .kind(),
        GuardErrorKind::NotDirectory
    );
    for protected in [".git/config", ".orchester/orchester.jsonc"] {
        assert_eq!(
            guard
                .resolve_write(Path::new(protected))
                .unwrap_err()
                .kind(),
            GuardErrorKind::Protected
        );
    }
}

#[test]
fn rejects_link_or_reparse_components_for_existing_and_future_targets() {
    let ws = TempWorkspace::new();
    fs::write(ws.outside.join("secret.txt"), "secret").unwrap();
    let link = ws.root.join("jump");
    if let Err(error) = create_directory_link(&ws.outside, &link) {
        if error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("skipping link test: {error}");
            return;
        }
        panic!("create directory link: {error}");
    }
    let guard = ws.guard();

    assert_eq!(
        guard
            .resolve_read(Path::new("jump/secret.txt"))
            .unwrap_err()
            .kind(),
        GuardErrorKind::LinkTraversal
    );
    assert_eq!(
        guard
            .resolve_write(Path::new("jump/new.rs"))
            .unwrap_err()
            .kind(),
        GuardErrorKind::LinkTraversal
    );

    remove_directory_link(&link).unwrap();
}

#[test]
fn filesystem_resolver_rejects_a_link_in_the_existing_parent_chain() {
    let ws = TempWorkspace::new();
    fs::create_dir(ws.outside.join("real")).unwrap();
    let link = ws.root.join("jump");
    if let Err(error) = create_directory_link(&ws.outside, &link) {
        if error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("skipping link test: {error}");
            return;
        }
        panic!("create directory link: {error}");
    }

    let error = FilesystemResolver
        .resolve_parent_no_links(&link.join("real/new.rs"))
        .unwrap_err();
    assert_eq!(error.kind(), GuardErrorKind::LinkTraversal);
    remove_directory_link(&link).unwrap();
}

#[test]
fn revalidation_detects_parent_replacement_before_write() {
    let ws = TempWorkspace::new();
    let safe = ws.root.join("safe");
    fs::create_dir(&safe).unwrap();
    let guard = ws.guard();
    let resolved = guard.resolve_write(Path::new("safe/new.rs")).unwrap();

    fs::rename(&safe, ws.root.join("safe-old")).unwrap();
    std::thread::sleep(Duration::from_millis(2));
    fs::create_dir(&safe).unwrap();

    assert_eq!(
        resolved.revalidate(&guard).unwrap_err().kind(),
        GuardErrorKind::Changed
    );
}

#[test]
fn revalidation_rejects_link_inserted_after_initial_resolution() {
    let ws = TempWorkspace::new();
    let safe = ws.root.join("safe");
    fs::create_dir(&safe).unwrap();
    let guard = ws.guard();
    let resolved = guard.resolve_write(Path::new("safe/new.rs")).unwrap();

    fs::rename(&safe, ws.root.join("safe-old")).unwrap();
    if let Err(error) = create_directory_link(&ws.outside, &safe) {
        if error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("skipping link test: {error}");
            return;
        }
        panic!("create directory link: {error}");
    }

    assert_eq!(
        resolved.revalidate(&guard).unwrap_err().kind(),
        GuardErrorKind::LinkTraversal
    );
    remove_directory_link(&safe).unwrap();
}

#[test]
fn atomic_write_target_is_a_new_regular_file_in_the_verified_parent() {
    let ws = TempWorkspace::new();
    let guard = ws.guard();
    let resolved = guard.resolve_write(Path::new("src/new.rs")).unwrap();

    let target = guard.atomic_write_target(&resolved).unwrap();
    assert_eq!(
        target.path().parent(),
        Some(resolved.final_path.parent().unwrap())
    );
    let metadata = fs::symlink_metadata(target.path()).unwrap();
    assert!(metadata.file_type().is_file());
    assert!(!metadata.file_type().is_symlink());
}

#[tokio::test]
async fn workspace_lock_serializes_mutations_for_the_same_identity() {
    let locks = WorkspaceLocks::default();
    let first = locks.mutate("workspace-a").await;
    let contender_locks = locks.clone();
    let contender = tokio::spawn(async move {
        let _guard = contender_locks.mutate("workspace-a").await;
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!contender.is_finished());
    drop(first);
    tokio::time::timeout(Duration::from_secs(1), contender)
        .await
        .expect("contender should acquire after release")
        .expect("contender task");
}

#[tokio::test]
async fn workspace_lock_allows_reads_and_keeps_identities_independent() {
    let locks = WorkspaceLocks::default();
    let read = locks.read("workspace-a").await;
    let second_read = tokio::time::timeout(Duration::from_secs(1), locks.read("workspace-a"))
        .await
        .expect("parallel read");
    let other_mutation = tokio::time::timeout(Duration::from_secs(1), locks.mutate("workspace-b"))
        .await
        .expect("independent mutation");

    drop((read, second_read, other_mutation));
}

#[tokio::test]
async fn mutation_resolution_waits_for_the_workspace_lock() {
    let ws = TempWorkspace::new();
    let safe = ws.root.join("safe");
    fs::create_dir(&safe).unwrap();
    let locks = WorkspaceLocks::default();
    let first = locks.mutate("workspace-a").await;
    let contender_locks = locks.clone();
    let root = ws.root.clone();
    let contender = tokio::spawn(async move {
        let guard = WorkspaceGuard::new(root).expect("workspace guard");
        contender_locks
            .resolve_mutation("workspace-a", &guard, Path::new("safe/new.rs"))
            .await
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!contender.is_finished());
    fs::rename(&safe, ws.root.join("safe-old")).unwrap();
    if let Err(error) = create_directory_link(&ws.outside, &safe) {
        if error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("skipping link test: {error}");
            drop(first);
            let _ = contender.await;
            return;
        }
        panic!("create directory link: {error}");
    }
    drop(first);

    let result = tokio::time::timeout(Duration::from_secs(1), contender)
        .await
        .expect("contender should resolve after release")
        .expect("contender task");
    assert_eq!(result.unwrap_err().kind(), GuardErrorKind::LinkTraversal);
    remove_directory_link(&safe).unwrap();
}

#[cfg(unix)]
fn create_directory_link(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_link(target: &Path, link: &Path) -> io::Result<()> {
    match std::os::windows::fs::symlink_dir(target, link) {
        Ok(()) => Ok(()),
        Err(symlink_error) => {
            let status = Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .status()?;
            if status.success() {
                Ok(())
            } else if symlink_error.raw_os_error() == Some(1314) {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    symlink_error,
                ))
            } else {
                Err(symlink_error)
            }
        }
    }
}

#[cfg(unix)]
fn remove_directory_link(link: &Path) -> io::Result<()> {
    fs::remove_file(link)
}

#[cfg(windows)]
fn remove_directory_link(link: &Path) -> io::Result<()> {
    fs::remove_dir(link)
}
