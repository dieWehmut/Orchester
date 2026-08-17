use std::ffi::{c_void, OsStr, OsString};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows_sys::Win32::System::Console::{ClosePseudoConsole, CreatePseudoConsole, COORD, HPCON};
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

const READ_CHUNK_BYTES: usize = 16 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct ConPty {
    pseudo_console: HPCON,
    input: HANDLE,
    output: HANDLE,
    process: HANDLE,
    captured: Vec<u8>,
}

impl ConPty {
    pub fn spawn(
        application: &Path,
        cwd: &Path,
        environment: &[(OsString, OsString)],
        columns: i16,
        rows: i16,
    ) -> io::Result<Self> {
        if columns <= 0 || rows <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ConPTY dimensions must be positive",
            ));
        }

        let mut pseudo_input = null_mut();
        let mut input = null_mut();
        let mut output = null_mut();
        let mut pseudo_output = null_mut();
        unsafe {
            if CreatePipe(&mut pseudo_input, &mut input, null(), 0) == 0 {
                return Err(io::Error::last_os_error());
            }
            if CreatePipe(&mut output, &mut pseudo_output, null(), 0) == 0 {
                close_handle(pseudo_input);
                close_handle(input);
                return Err(io::Error::last_os_error());
            }
        }

        let mut pseudo_console = 0;
        let create_result = unsafe {
            CreatePseudoConsole(
                COORD {
                    X: columns,
                    Y: rows,
                },
                pseudo_input,
                pseudo_output,
                0,
                &mut pseudo_console,
            )
        };
        if create_result < 0 {
            unsafe {
                close_handle(pseudo_input);
                close_handle(pseudo_output);
                close_handle(input);
                close_handle(output);
            }
            return Err(io::Error::from_raw_os_error(create_result));
        }

        let mut attribute_bytes = 0_usize;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut attribute_bytes);
        }
        if attribute_bytes == 0 {
            unsafe {
                close_handle(pseudo_input);
                close_handle(pseudo_output);
                close_handle(input);
                close_handle(output);
                ClosePseudoConsole(pseudo_console);
            }
            return Err(io::Error::last_os_error());
        }
        let word = std::mem::size_of::<usize>();
        let mut attribute_storage = vec![0_usize; attribute_bytes.div_ceil(word)];
        let attribute_list = attribute_storage.as_mut_ptr().cast();
        let initialized = unsafe {
            InitializeProcThreadAttributeList(attribute_list, 1, 0, &mut attribute_bytes)
        };
        if initialized == 0 {
            unsafe {
                close_handle(pseudo_input);
                close_handle(pseudo_output);
                close_handle(input);
                close_handle(output);
                ClosePseudoConsole(pseudo_console);
            }
            return Err(io::Error::last_os_error());
        }
        let updated = unsafe {
            UpdateProcThreadAttribute(
                attribute_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                pseudo_console as *const c_void,
                std::mem::size_of::<HPCON>(),
                null_mut(),
                null(),
            )
        };
        if updated == 0 {
            unsafe {
                DeleteProcThreadAttributeList(attribute_list);
                close_handle(pseudo_input);
                close_handle(pseudo_output);
                close_handle(input);
                close_handle(output);
                ClosePseudoConsole(pseudo_console);
            }
            return Err(io::Error::last_os_error());
        }

        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.lpAttributeList = attribute_list;
        let application = wide_null(application.as_os_str());
        let mut command_line = quoted_command_line(application_path(application.as_slice()));
        let cwd = wide_null(cwd.as_os_str());
        let environment = (!environment.is_empty()).then(|| environment_block(environment));
        let environment_pointer = environment
            .as_ref()
            .map_or(null(), |block| block.as_ptr().cast());
        let creation_flags = EXTENDED_STARTUPINFO_PRESENT
            | if environment.is_some() {
                CREATE_UNICODE_ENVIRONMENT
            } else {
                0
            };
        let mut process_info = PROCESS_INFORMATION::default();
        let created = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                0,
                creation_flags,
                environment_pointer,
                cwd.as_ptr(),
                &startup.StartupInfo,
                &mut process_info,
            )
        };
        unsafe {
            close_handle(pseudo_input);
            close_handle(pseudo_output);
            DeleteProcThreadAttributeList(attribute_list);
        }
        if created == 0 {
            unsafe {
                close_handle(input);
                close_handle(output);
                ClosePseudoConsole(pseudo_console);
            }
            return Err(io::Error::last_os_error());
        }
        unsafe {
            close_handle(process_info.hThread);
        }

        Ok(Self {
            pseudo_console,
            input,
            output,
            process: process_info.hProcess,
            captured: Vec::new(),
        })
    }

    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        let mut offset = 0;
        while offset < bytes.len() {
            let remaining = &bytes[offset..];
            let mut written = 0;
            let ok = unsafe {
                WriteFile(
                    self.input,
                    remaining.as_ptr(),
                    remaining.len().min(u32::MAX as usize) as u32,
                    &mut written,
                    null_mut(),
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "ConPTY input pipe accepted no bytes",
                ));
            }
            offset += written as usize;
        }
        Ok(())
    }

    pub fn read_until(&mut self, marker: &[u8], timeout: Duration) -> io::Result<usize> {
        let start = self.captured.len();
        self.read_until_since(start, marker, timeout)
    }

    pub fn read_until_since(
        &mut self,
        start: usize,
        marker: &[u8],
        timeout: Duration,
    ) -> io::Result<usize> {
        let deadline = Instant::now() + timeout;
        loop {
            self.read_available()?;
            if contains_bytes(&self.captured[start.min(self.captured.len())..], marker) {
                return Ok(self.captured.len());
            }
            if unsafe { WaitForSingleObject(self.process, 0) } == WAIT_OBJECT_0 {
                self.finish_output()?;
                if contains_bytes(&self.captured[start.min(self.captured.len())..], marker) {
                    return Ok(self.captured.len());
                }
                let mut exit_code = 0;
                unsafe {
                    GetExitCodeProcess(self.process, &mut exit_code);
                }
                let preview = String::from_utf8_lossy(&self.captured);
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "process exited with {exit_code} before marker {:?}; output: {}",
                        String::from_utf8_lossy(marker),
                        preview.chars().take(2_000).collect::<String>()
                    ),
                ));
            }
            if Instant::now() >= deadline {
                let start = self.captured.len().saturating_sub(4_000);
                let preview = String::from_utf8_lossy(&self.captured[start..]);
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "timed out waiting for marker {:?}; output tail: {}",
                        String::from_utf8_lossy(marker),
                        preview.chars().take(4_000).collect::<String>()
                    ),
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    pub fn wait_for_exit(mut self, timeout: Duration) -> io::Result<(u32, Vec<u8>)> {
        let deadline = Instant::now() + timeout;
        loop {
            self.read_available()?;
            match unsafe { WaitForSingleObject(self.process, 0) } {
                WAIT_OBJECT_0 => break,
                WAIT_TIMEOUT if Instant::now() < deadline => {
                    std::thread::sleep(POLL_INTERVAL);
                }
                WAIT_TIMEOUT => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "timed out waiting for ConPTY child",
                    ));
                }
                _ => return Err(io::Error::last_os_error()),
            }
        }
        self.finish_output()?;
        let mut exit_code = 0;
        if unsafe { GetExitCodeProcess(self.process, &mut exit_code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        self.close();
        Ok((exit_code, std::mem::take(&mut self.captured)))
    }

    fn read_available(&mut self) -> io::Result<()> {
        loop {
            let mut available = 0;
            let ok = unsafe {
                PeekNamedPipe(
                    self.output,
                    null_mut(),
                    0,
                    null_mut(),
                    &mut available,
                    null_mut(),
                )
            };
            if ok == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(109) {
                    return Ok(());
                }
                return Err(error);
            }
            if available == 0 {
                return Ok(());
            }
            let mut buffer = vec![0_u8; READ_CHUNK_BYTES.min(available as usize)];
            let mut read = 0;
            let ok = unsafe {
                ReadFile(
                    self.output,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    &mut read,
                    null_mut(),
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            buffer.truncate(read as usize);
            self.captured.extend_from_slice(&buffer);
        }
    }

    fn finish_output(&mut self) -> io::Result<()> {
        if self.pseudo_console == 0 || self.output.is_null() {
            return Ok(());
        }

        unsafe {
            close_handle(self.input);
        }
        self.input = null_mut();
        let pseudo_console = std::mem::replace(&mut self.pseudo_console, 0);
        let closer = std::thread::spawn(move || unsafe {
            ClosePseudoConsole(pseudo_console);
        });
        let read_result = self.read_output_to_eof();
        unsafe {
            close_handle(self.output);
        }
        self.output = null_mut();
        closer
            .join()
            .map_err(|_| io::Error::other("ConPTY close thread panicked"))?;
        read_result
    }

    fn read_output_to_eof(&mut self) -> io::Result<()> {
        loop {
            let mut buffer = vec![0_u8; READ_CHUNK_BYTES];
            let mut read = 0;
            let ok = unsafe {
                ReadFile(
                    self.output,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    &mut read,
                    null_mut(),
                )
            };
            if ok == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(109) {
                    return Ok(());
                }
                return Err(error);
            }
            if read == 0 {
                return Ok(());
            }
            buffer.truncate(read as usize);
            self.captured.extend_from_slice(&buffer);
        }
    }

    fn close(&mut self) {
        unsafe {
            close_handle(self.input);
            close_handle(self.output);
            if self.pseudo_console != 0 {
                ClosePseudoConsole(self.pseudo_console);
                self.pseudo_console = 0;
            }
            close_handle(self.process);
        }
        self.input = null_mut();
        self.output = null_mut();
        self.process = null_mut();
    }
}

impl Drop for ConPty {
    fn drop(&mut self) {
        if !self.process.is_null()
            && unsafe { WaitForSingleObject(self.process, 0) } == WAIT_TIMEOUT
        {
            unsafe {
                TerminateProcess(self.process, 1);
                WaitForSingleObject(self.process, 2_000);
            }
        }
        self.close();
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn application_path(value: &[u16]) -> OsString {
    OsString::from(String::from_utf16_lossy(
        value.strip_suffix(&[0]).unwrap_or(value),
    ))
}

fn quoted_command_line(application: OsString) -> Vec<u16> {
    let mut value = OsString::from("\"");
    value.push(application);
    value.push("\"");
    wide_null(&value)
}

fn environment_block(overrides: &[(OsString, OsString)]) -> Vec<u16> {
    let mut values = std::env::vars_os()
        .filter(|(key, _)| {
            !overrides.iter().any(|(override_key, _)| {
                key.to_string_lossy()
                    .eq_ignore_ascii_case(&override_key.to_string_lossy())
            })
        })
        .collect::<Vec<_>>();
    values.extend(overrides.iter().cloned());
    values.sort_by(|(left, _), (right, _)| {
        left.to_string_lossy()
            .to_ascii_uppercase()
            .cmp(&right.to_string_lossy().to_ascii_uppercase())
    });
    let mut block = Vec::new();
    for (key, value) in values {
        let mut entry = key;
        entry.push("=");
        entry.push(value);
        block.extend(entry.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

unsafe fn close_handle(handle: HANDLE) {
    if !handle.is_null() {
        CloseHandle(handle);
    }
}
