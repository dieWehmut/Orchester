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
pub(crate) struct ProcessTree {
    #[cfg(windows)]
    job: OwnedHandle,
}

impl ProcessTree {
    pub(crate) fn new() -> io::Result<Self> {
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

    pub(crate) fn configure_command(&self, command: &mut Command) {
        #[cfg(unix)]
        {
            command.process_group(0);
        }
        #[cfg(not(unix))]
        {
            let _ = command;
        }
    }

    pub(crate) fn attach(&self, child: &Child) -> io::Result<()> {
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

    pub(crate) fn terminate(&self, child: &mut Child) {
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
