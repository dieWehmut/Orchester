//! Real Windows subprocesses exercise the capture helper, without starting WSL.
use super::{command_output_result, WslReadFailure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessesToUpdate, System};

const MODE: &str = "ORCHESTER_WSL_LIFECYCLE_TEST_MODE";
const ROOT: &str = "ORCHESTER_WSL_LIFECYCLE_TEST_ROOT";
const PROBE: &str = "agent_activity::wsl::windows_lifecycle_tests::windows_helper_probe";

fn probe_command(root: &Path, mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", PROBE, "--nocapture"])
        .env(MODE, mode)
        .env(ROOT, root);
    command
}

fn wait_file(root: &Path, file: &str) {
    let deadline = Instant::now() + Duration::from_secs(12);
    while !root.join(file).exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(root.join(file).exists(), "fixture did not produce {file}");
}

fn wait_release(root: &Path, file: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !root.join(file).exists() && !root.join("stop").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn windows_helper_probe() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
    match mode.as_str() {
        "leaf" => {
            fs::write(root.join("leaf.pid"), std::process::id().to_string()).unwrap();
            wait_release(&root, "stop");
        }
        "relay" => {
            fs::write(root.join("relay.pid"), std::process::id().to_string()).unwrap();
            let mut leaf = probe_command(&root, "leaf").spawn().unwrap();
            let _ = leaf.wait();
        }
        "unrelated" => wait_release(&root, "unrelated-stop"),
        "host" => {
            let owned_root = root.clone();
            thread::spawn(move || {
                let _ = command_output_result(
                    &mut probe_command(&owned_root, "timeout"),
                    Duration::from_secs(30),
                    64 * 1024,
                );
            });
            wait_file(&root, "tree.ready");
            fs::write(root.join("host.ready"), "ready").unwrap();
            wait_release(&root, "host-exit");
            // Match backend termination: the detached worker gets no Rust Drop.
            std::process::exit(0);
        }
        "success" | "failed" | "timeout" | "output-limit" => {
            fs::write(root.join("wrapper.pid"), std::process::id().to_string()).unwrap();
            // Descendants start immediately; no post-spawn attachment handshake.
            let mut relay = probe_command(&root, "relay").spawn().unwrap();
            wait_file(&root, "leaf.pid");
            fs::write(root.join("tree.ready"), "ready").unwrap();
            wait_release(&root, "release");
            match mode.as_str() {
                "success" => {
                    println!("OBSERVED_WSL_HELPER_OUTPUT");
                    std::process::exit(0);
                }
                "failed" => std::process::exit(7),
                "output-limit" => println!("{}", "x".repeat(8192)),
                _ => {}
            }
            wait_release(&root, "stop");
            let _ = relay.wait();
        }
        _ => panic!("unknown isolated fixture mode"),
    }
}

fn observed_processes(root: &Path) -> Vec<Pid> {
    ["wrapper.pid", "relay.pid", "leaf.pid"]
        .into_iter()
        .filter_map(|file| fs::read_to_string(root.join(file)).ok())
        .map(|pid| Pid::from_u32(pid.parse().unwrap()))
        .collect()
}

fn any_alive(pids: &[Pid]) -> bool {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(pids), true);
    pids.iter().any(|pid| system.process(*pid).is_some())
}

fn assert_complete_live_tree(root: &Path) {
    let pids = observed_processes(root);
    assert_eq!(pids.len(), 3, "fixture did not start the complete tree");
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::Some(&pids), true);
    assert!(pids.iter().all(|pid| system.process(*pid).is_some()));
}

struct OwnedHost(Child);
impl Drop for OwnedHost {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Fixture {
    root: tempfile::TempDir,
    unrelated: Child,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let unrelated = probe_command(root.path(), "unrelated")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Self { root, unrelated }
    }
    fn assert_tree_gone(&mut self) {
        let pids = observed_processes(self.root.path());
        assert_eq!(pids.len(), 3, "fixture did not start the complete tree");
        let deadline = Instant::now() + Duration::from_secs(5);
        while any_alive(&pids) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !any_alive(&pids),
            "owned Windows helper descendants survived"
        );
        assert!(self.unrelated.try_wait().unwrap().is_none());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Failure cleanup is cooperative and scoped to this unique fixture.
        // Never signal a descendant or unrelated process by a discovered PID.
        let _ = fs::write(self.root.path().join("stop"), "stop");
        let pids = observed_processes(self.root.path());
        let deadline = Instant::now() + Duration::from_secs(5);
        while any_alive(&pids) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        if any_alive(&pids) {
            eprintln!("isolated WSL helper fixture cooperative cleanup timed out");
        }
        let _ = self.unrelated.kill();
        let _ = self.unrelated.wait();
    }
}

fn capture_case(mode: &str, expected: Result<(), WslReadFailure>) {
    let mut fixture = Fixture::new();
    let root = fixture.root.path().to_owned();
    let worker_root = root.clone();
    let timeout = if mode == "timeout" {
        Duration::from_secs(3)
    } else {
        Duration::from_secs(15)
    };
    let limit = if mode == "output-limit" {
        1024
    } else {
        64 * 1024
    };
    let mode = mode.to_owned();
    let worker = thread::spawn(move || {
        command_output_result(&mut probe_command(&worker_root, &mode), timeout, limit)
    });
    wait_file(&root, "tree.ready");
    assert_complete_live_tree(&root);
    fs::write(root.join("release"), "release").unwrap();
    let result = worker.join().unwrap();
    if expected.is_ok() {
        assert!(String::from_utf8(result.clone().unwrap())
            .unwrap()
            .contains("OBSERVED_WSL_HELPER_OUTPUT"));
    }
    assert_eq!(result.map(|_| ()), expected);
    fixture.assert_tree_gone();
}

#[test]
fn success_cleans_windows_wrapper_descendants() {
    capture_case("success", Ok(()));
}
#[test]
fn failure_cleans_windows_wrapper_descendants() {
    capture_case("failed", Err(WslReadFailure::CommandFailed));
}
#[test]
fn timeout_cleans_windows_wrapper_descendants() {
    capture_case("timeout", Err(WslReadFailure::Timeout));
}
#[test]
fn output_limit_cleans_windows_wrapper_descendants() {
    capture_case("output-limit", Err(WslReadFailure::OutputLimit));
}
#[test]
fn backend_exit_cleans_detached_windows_helper_worker() {
    let mut fixture = Fixture::new();
    let mut host = OwnedHost(
        probe_command(fixture.root.path(), "host")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_file(fixture.root.path(), "host.ready");
    assert_complete_live_tree(fixture.root.path());
    fs::write(fixture.root.path().join("host-exit"), "exit").unwrap();
    assert!(host.0.wait().unwrap().success());
    fixture.assert_tree_gone();
}
