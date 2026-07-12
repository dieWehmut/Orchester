#[cfg(unix)]
use std::ffi::CString;
use std::fs;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::net::UnixListener;
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
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

#[cfg(windows)]
#[test]
fn rejects_ntfs_alternate_data_stream_paths() {
    let ws = TempWorkspace::new();
    let guard = ws.guard();
    let path = Path::new("src/value.txt:orchester-hidden");

    let error = guard
        .resolve_write(path)
        .expect_err("alternate data streams must not enter the workspace capability model");
    assert_eq!(error.kind(), GuardErrorKind::InvalidPath);
    assert!(!ws.root.join(path).exists());
}

#[cfg(windows)]
#[test]
fn rejects_win32_aliases_and_reserved_device_names() {
    let ws = TempWorkspace::new();
    let guard = ws.guard();

    assert_eq!(
        guard
            .resolve_write(Path::new(".GIT/config"))
            .unwrap_err()
            .kind(),
        GuardErrorKind::Protected
    );
    for path in [
        ".git.",
        ".orchester ",
        "src/value.txt.",
        "src/value.txt ",
        "src/CON",
        "src/con.txt",
        "src/NUL.rs",
        "src/LPT1.log",
        "src/bad?.txt",
    ] {
        assert_eq!(
            guard.resolve_write(Path::new(path)).unwrap_err().kind(),
            GuardErrorKind::InvalidPath,
            "accepted Win32-ambiguous path {path:?}"
        );
    }
}

#[test]
fn workspace_root_open_rejects_link_or_reparse_components() {
    let ws = TempWorkspace::new();
    let link = ws.root.parent().unwrap().join(format!(
        "workspace-root-link-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&link);
    if let Err(error) = create_directory_link(&ws.root, &link) {
        if error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("skipping root link test: {error}");
            return;
        }
        panic!("create workspace root link: {error}");
    }

    let error = WorkspaceGuard::new(&link)
        .expect_err("workspace root must be opened component-by-component without links");
    assert_eq!(error.kind(), GuardErrorKind::LinkTraversal);
    let _ = fs::remove_dir_all(link);
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

#[cfg(unix)]
#[test]
fn rejects_non_regular_file_types_without_opening_them() {
    let ws = TempWorkspace::new();
    let _socket = UnixListener::bind(ws.root.join("src/orchester.sock")).unwrap();
    fs::write(ws.root.join("src/value.txt"), "inside").unwrap();
    let guard = ws.guard();

    assert_eq!(
        guard
            .resolve_read(Path::new("src/orchester.sock"))
            .unwrap_err()
            .kind(),
        GuardErrorKind::Unsupported
    );
    assert_eq!(
        guard
            .rename(Path::new("src/value.txt"), Path::new("src/orchester.sock"),)
            .unwrap_err()
            .kind(),
        GuardErrorKind::Unsupported
    );
    assert!(ws.root.join("src/value.txt").is_file());
}

#[cfg(unix)]
#[test]
fn fifo_read_is_rejected_without_blocking() {
    let ws = TempWorkspace::new();
    let fifo = ws.root.join("src/orchester.fifo");
    let raw_path = CString::new(fifo.as_os_str().as_bytes()).expect("fifo path has no NUL");
    let result = unsafe { libc::mkfifo(raw_path.as_ptr(), 0o600) };
    assert_eq!(result, 0, "mkfifo: {}", io::Error::last_os_error());

    let guard = ws.guard();
    let started = std::time::Instant::now();
    let error = guard
        .read_file(Path::new("src/orchester.fifo"))
        .expect_err("FIFO must never be treated as a regular source file");
    assert_eq!(error.kind(), GuardErrorKind::Unsupported);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "FIFO rejection took too long: {:?}",
        started.elapsed()
    );
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

#[test]
fn atomic_commit_rejects_a_replaced_destination_object() {
    let ws = TempWorkspace::new();
    fs::write(ws.root.join("src/value.rs"), "original").unwrap();
    let guard = ws.guard();
    let resolved = guard.resolve_write(Path::new("src/value.rs")).unwrap();
    let mut target = guard.atomic_write_target(&resolved).unwrap();
    target.file_mut().write_all(b"generated").unwrap();

    fs::rename(
        ws.root.join("src/value.rs"),
        ws.root.join("src/value-old.rs"),
    )
    .unwrap();
    fs::write(ws.root.join("src/value.rs"), "replacement").unwrap();

    assert_eq!(target.commit().unwrap_err().kind(), GuardErrorKind::Changed);
    assert_eq!(
        fs::read_to_string(ws.root.join("src/value.rs")).unwrap(),
        "replacement"
    );
}

#[tokio::test]
async fn workspace_lock_serializes_mutations_for_the_same_workspace() {
    let ws = TempWorkspace::new();
    let first_guard = ws.guard();
    let second_guard = ws.guard();
    let locks = WorkspaceLocks::default();
    let first = locks.mutate(&first_guard).await;
    let contender_locks = locks.clone();
    let contender = tokio::spawn(async move {
        let _guard = contender_locks.mutate(&second_guard).await;
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
    let ws_a = TempWorkspace::new();
    let ws_b = TempWorkspace::new();
    let guard_a = ws_a.guard();
    let guard_b = ws_b.guard();
    let locks = WorkspaceLocks::default();
    let read = locks.read(&guard_a).await;
    let second_read = tokio::time::timeout(Duration::from_secs(1), locks.read(&guard_a))
        .await
        .expect("parallel read");
    let other_mutation = tokio::time::timeout(Duration::from_secs(1), locks.mutate(&guard_b))
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
    let first_guard = ws.guard();
    let first = locks.mutate(&first_guard).await;
    let contender_locks = locks.clone();
    let root = ws.root.clone();
    let contender = tokio::spawn(async move {
        let guard = WorkspaceGuard::new(root).expect("workspace guard");
        contender_locks
            .resolve_mutation(&guard, Path::new("safe/new.rs"))
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

#[cfg(windows)]
#[test]
fn revalidation_rejects_replacement_with_matching_legacy_metadata() {
    let ws = TempWorkspace::new();
    let safe = ws.root.join("safe");
    fs::create_dir(&safe).unwrap();
    let guard = ws.guard();
    let resolved = guard.resolve_write(Path::new("safe/new.rs")).unwrap();
    let original_creation_time = fs::metadata(&safe).unwrap().creation_time();

    fs::rename(&safe, ws.root.join("safe-old")).unwrap();
    fs::create_dir(&safe).unwrap();
    set_creation_time(&safe, original_creation_time).unwrap();

    assert_eq!(
        resolved.revalidate(&guard).unwrap_err().kind(),
        GuardErrorKind::Changed
    );
}

#[test]
fn guarded_read_does_not_follow_a_parent_swap_race() {
    let ws = TempWorkspace::new();
    fs::create_dir(ws.root.join("safe-real")).unwrap();
    fs::write(ws.root.join("safe-real/value.txt"), "inside").unwrap();
    fs::write(ws.outside.join("value.txt"), "secret").unwrap();
    let link = ws.root.join("safe-link");
    if let Err(error) = create_directory_link(&ws.outside, &link) {
        if error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("skipping race test: {error}");
            return;
        }
        panic!("create directory link: {error}");
    }
    fs::rename(ws.root.join("safe-real"), ws.root.join("safe")).unwrap();
    let guard = Arc::new(ws.guard());
    assert_eq!(
        guard.read_file(Path::new("safe/value.txt")).unwrap(),
        b"inside"
    );
    let stop = Arc::new(AtomicBool::new(false));
    let attacker = spawn_parent_swapper(ws.root.clone(), stop.clone());

    for _ in 0..200 {
        if let Ok(contents) = guard.read_file(Path::new("safe/value.txt")) {
            assert_eq!(contents, b"inside");
        }
    }
    stop.store(true, Ordering::Release);
    attacker.join().expect("race thread");
    assert_eq!(
        fs::read_to_string(ws.outside.join("value.txt")).unwrap(),
        "secret"
    );
}

#[test]
fn guarded_write_does_not_create_files_through_a_parent_swap_race() {
    let ws = TempWorkspace::new();
    fs::create_dir(ws.root.join("safe-real")).unwrap();
    fs::write(ws.outside.join("value.txt"), "secret").unwrap();
    let link = ws.root.join("safe-link");
    if let Err(error) = create_directory_link(&ws.outside, &link) {
        if error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("skipping race test: {error}");
            return;
        }
        panic!("create directory link: {error}");
    }
    fs::rename(ws.root.join("safe-real"), ws.root.join("safe")).unwrap();
    let guard = Arc::new(ws.guard());
    let stop = Arc::new(AtomicBool::new(false));
    let attacker = spawn_parent_swapper(ws.root.clone(), stop.clone());

    for _ in 0..200 {
        let _ = guard.write_atomic(Path::new("safe/output.txt"), b"inside");
    }
    stop.store(true, Ordering::Release);
    attacker.join().expect("race thread");
    assert_eq!(
        fs::read_to_string(ws.outside.join("value.txt")).unwrap(),
        "secret"
    );
    assert!(!ws.outside.join("output.txt").exists());
}

#[test]
fn guarded_rename_uses_verified_parent_handles() {
    let ws = TempWorkspace::new();
    fs::create_dir(ws.root.join("safe")).unwrap();
    fs::write(ws.root.join("safe/source.txt"), "inside").unwrap();
    let guard = ws.guard();

    guard
        .rename(Path::new("safe/source.txt"), Path::new("safe/dest.txt"))
        .unwrap();
    assert_eq!(
        fs::read_to_string(ws.root.join("safe/dest.txt")).unwrap(),
        "inside"
    );
    assert!(!ws.root.join("safe/source.txt").exists());
}

fn spawn_parent_swapper(root: PathBuf, stop: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let safe = root.join("safe");
        let swap = root.join("safe-swap");
        let link = root.join("safe-link");
        while !stop.load(Ordering::Acquire) {
            if fs::rename(&safe, &swap).is_err() {
                continue;
            }
            if fs::rename(&link, &safe).is_ok() {
                let _ = fs::rename(&safe, &link);
            }
            let _ = fs::rename(&swap, &safe);
        }
        let _ = fs::remove_dir(&safe);
        let _ = fs::rename(&swap, &safe);
    })
}

#[cfg(windows)]
fn set_creation_time(path: &Path, timestamp: u64) -> io::Result<()> {
    let script = format!(
        "$item = Get-Item -LiteralPath '{}'; $item.CreationTimeUtc = [DateTime]::FromFileTimeUtc({timestamp})",
        path.display().to_string().replace('\'', "''")
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("PowerShell could not set creation time"))
    }
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
