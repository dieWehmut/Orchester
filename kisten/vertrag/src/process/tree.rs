use std::io;

use tokio::process::{Child, Command};

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

#[cfg(windows)]
use windows_sys::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

/// Owns the operating-system primitive used to terminate a command and its
/// descendants as one unit.
pub struct ProcessTree {
    #[cfg(windows)]
    job: OwnedHandle,
}

impl ProcessTree {
    pub fn new() -> io::Result<Self> {
        #[cfg(windows)]
        {
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = unsafe { OwnedHandle::from_raw_handle(handle as _) };
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = unsafe {
                SetInformationJobObject(
                    job.as_raw_handle() as HANDLE,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if configured == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { job })
        }

        #[cfg(not(windows))]
        Ok(Self {})
    }

    pub fn configure_command(&self, command: &mut Command) {
        #[cfg(unix)]
        {
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            // This runtime captures all output; a GUI parent must not create a
            // separate console for each tool invocation.
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = command;
        }
    }

    pub fn attach(&self, child: &Child) -> io::Result<()> {
        #[cfg(windows)]
        {
            let process = child.raw_handle().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "child process already exited")
            })?;
            let assigned = unsafe {
                AssignProcessToJobObject(self.job.as_raw_handle() as HANDLE, process as HANDLE)
            };
            if assigned == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        #[cfg(not(windows))]
        {
            let _ = child;
        }
        Ok(())
    }

    pub fn terminate(&self, child: &mut Child) {
        #[cfg(windows)]
        {
            let _ = unsafe { TerminateJobObject(self.job.as_raw_handle() as HANDLE, 1) };
        }
        #[cfg(unix)]
        {
            if let Some(pid) = child.id().and_then(|pid| i32::try_from(pid).ok()) {
                if pid > 1 {
                    unsafe {
                        libc::kill(-pid, libc::SIGKILL);
                    }
                }
            }
        }
        let _ = child.start_kill();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        process::Stdio,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
    };

    const PROBE: &str = "ORCHESTER_PROCESS_TREE_PROBE";
    const ROOT: &str = "ORCHESTER_PROCESS_TREE_PROBE_ROOT";
    const TEST: &str = "process::tree::tests::job_probe";

    #[test]
    fn job_probe() {
        let Ok(mode) = std::env::var(PROBE) else {
            return;
        };
        let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
        if mode == "parent" {
            // Start the descendant only after the test attached this parent to
            // its job, so the assertion exercises inherited membership.
            for _ in 0..500 {
                if root.join("start").exists() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(root.join("start").exists());
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(PROBE, "descendant")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            fs::write(root.join("descendant.pid"), child.id().to_string()).unwrap();
            let _ = child.wait();
        } else {
            fs::write(root.join("ready"), "ready").unwrap();
            std::thread::sleep(Duration::from_secs(30));
        }
    }

    #[tokio::test]
    async fn termination_stops_windows_child_and_descendant() {
        let root = std::env::temp_dir().join(format!(
            "orchester-job-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let tree = ProcessTree::new().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", TEST, "--nocapture"])
            .env(PROBE, "parent")
            .env(ROOT, &root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        tree.configure_command(&mut command);
        let mut child = command.spawn().unwrap();
        tree.attach(&child).unwrap();
        fs::write(root.join("start"), "start").unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !root.join("ready").exists() || !root.join("descendant.pid").exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let pid: u32 = fs::read_to_string(root.join("descendant.pid"))
            .unwrap()
            .parse()
            .unwrap();
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        assert!(!handle.is_null());
        let descendant = unsafe { OwnedHandle::from_raw_handle(handle as _) };
        assert_eq!(
            unsafe { WaitForSingleObject(descendant.as_raw_handle() as HANDLE, 0) },
            WAIT_TIMEOUT
        );
        tree.terminate(&mut child);
        assert!(!tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success());
        assert_eq!(
            unsafe { WaitForSingleObject(descendant.as_raw_handle() as HANDLE, 5000) },
            WAIT_OBJECT_0
        );
        fs::remove_dir_all(root).unwrap();
    }
}
