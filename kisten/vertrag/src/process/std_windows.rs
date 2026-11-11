//! Stable std::process::Command has no stable initial-thread handle API.
//! A suspended child has not executed user code: locate its sole initial
//! thread, verify its owner using the opened thread handle, then resume only
//! after the original child process handle is assigned to our job.
use std::{
    io,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    process::{Child, Command},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE, WAIT_TIMEOUT},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        JobObjects::AssignProcessToJobObject,
        Threading::{
            GetProcessIdOfThread, OpenThread, ResumeThread, WaitForSingleObject, CREATE_NO_WINDOW,
            CREATE_SUSPENDED, THREAD_QUERY_LIMITED_INFORMATION, THREAD_SUSPEND_RESUME,
        },
    },
};

fn initial_thread(child: &Child) -> io::Result<OwnedHandle> {
    let process = child.as_raw_handle() as HANDLE;
    if unsafe { WaitForSingleObject(process, 0) } != WAIT_TIMEOUT {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "suspended child exited",
        ));
    }
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw as _) };
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..THREADENTRY32::default()
    };
    if unsafe { Thread32First(snapshot.as_raw_handle() as HANDLE, &mut entry) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut initial = None;
    loop {
        if entry.th32OwnerProcessID == child.id() {
            if initial.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "ambiguous initial thread",
                ));
            }
            let raw = unsafe {
                OpenThread(
                    THREAD_SUSPEND_RESUME | THREAD_QUERY_LIMITED_INFORMATION,
                    0,
                    entry.th32ThreadID,
                )
            };
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let thread = unsafe { OwnedHandle::from_raw_handle(raw as _) };
            // The process is still alive on its original owned handle, so its
            // PID cannot be recycled. A recycled thread ID must match this
            // process through the opened handle before we can resume it.
            if unsafe { GetProcessIdOfThread(thread.as_raw_handle() as HANDLE) } != child.id() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "initial thread owner changed",
                ));
            }
            initial = Some(thread);
        }
        if unsafe { Thread32Next(snapshot.as_raw_handle() as HANDLE, &mut entry) } == 0 {
            if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                return Err(io::Error::last_os_error());
            }
            break;
        }
    }
    if unsafe { WaitForSingleObject(process, 0) } != WAIT_TIMEOUT {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "suspended child exited",
        ));
    }
    initial.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "initial thread missing"))
}

pub(super) fn spawn_in_job(command: &mut Command, job: &OwnedHandle) -> io::Result<Child> {
    command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
    finish_spawn(command.spawn()?, job)
}

fn finish_spawn(mut child: Child, job: &OwnedHandle) -> io::Result<Child> {
    let setup = (|| {
        if unsafe {
            AssignProcessToJobObject(
                job.as_raw_handle() as HANDLE,
                child.as_raw_handle() as HANDLE,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let thread = initial_thread(&child)?;
        let previous = unsafe { ResumeThread(thread.as_raw_handle() as HANDLE) };
        if previous == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        if previous != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "initial suspend count changed",
            ));
        }
        Ok(())
    })();
    if let Err(error) = setup {
        // Assignment may fail: this never-executed child is still ours through
        // the original std handle, so terminate it without reopening any PID.
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        process::Stdio,
        time::{SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS, WAIT_OBJECT_0},
        System::Threading::{CreateEventW, GetCurrentProcess},
    };

    const MARKER: &str = "ORCHESTER_SUSPENDED_STD_PROBE_MARKER";
    const PROBE: &str = "process::std_windows::tests::suspended_start_probe";

    #[test]
    fn suspended_start_probe() {
        if let Some(path) = std::env::var_os(MARKER) {
            fs::write(path, "user code executed").unwrap();
        }
    }

    #[test]
    fn failed_job_assignment_reaps_original_child_without_running_user_code() {
        let marker = std::env::temp_dir().join(format!(
            "orchester-suspended-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", PROBE, "--nocapture"])
            .env(MARKER, &marker)
            .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut raw = std::ptr::null_mut();
        assert_ne!(
            unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    child.as_raw_handle() as HANDLE,
                    GetCurrentProcess(),
                    &mut raw,
                    0,
                    0,
                    DUPLICATE_SAME_ACCESS,
                )
            },
            0
        );
        let observer = unsafe { OwnedHandle::from_raw_handle(raw as _) };
        // A real event HANDLE deliberately fails the Win32 job-kind check.
        // It remains a valid owned kernel handle, rather than constructing an
        // invalid OwnedHandle or killing a process found by PID.
        let raw = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
        assert!(!raw.is_null());
        let not_a_job = unsafe { OwnedHandle::from_raw_handle(raw as _) };
        assert!(finish_spawn(child, &not_a_job).is_err());
        assert_eq!(
            unsafe { WaitForSingleObject(observer.as_raw_handle() as HANDLE, 5000) },
            WAIT_OBJECT_0
        );
        assert!(!marker.exists());
    }
}
