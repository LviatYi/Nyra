use std::{io, process::Child};

#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

/// Owned and released on the execution worker, including after a prior crash.
pub(super) struct ScriptSlot {
    #[cfg(windows)]
    handle: OwnedHandle,
}

impl ScriptSlot {
    pub(super) fn acquire() -> Result<Self, String> {
        #[cfg(windows)]
        unsafe {
            let name: Vec<u16> = "Local\\Nyra.Reaction.Script\0".encode_utf16().collect();
            let raw = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if raw.is_null() {
                return Err(format!(
                    "Unable to create mutex: {}",
                    io::Error::last_os_error()
                ));
            }
            let handle = OwnedHandle::from_raw_handle(raw);
            match WaitForSingleObject(raw, 0) {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self { handle }),
                WAIT_TIMEOUT => Err("A Nyra script is already running or cleaning up in the current Windows session".into()),
                _ => Err(format!("Unable to acquire script mutex: {}", io::Error::last_os_error())),
            }
        }
        #[cfg(not(windows))]
        Err("Reaction currently requires Windows".into())
    }
}

impl Drop for ScriptSlot {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            ReleaseMutex(self.handle.as_raw_handle());
        }
    }
}

/// Closing the job terminates the runner and any descendants holding its pipes.
pub(super) struct ProcessJob {
    #[cfg(windows)]
    _handle: OwnedHandle,
}

impl ProcessJob {
    pub(super) fn attach(child: &Child) -> io::Result<Self> {
        #[cfg(windows)]
        unsafe {
            // The handle is owned immediately and closed on every failure path.
            let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let handle = OwnedHandle::from_raw_handle(raw);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                raw,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(raw, child.as_raw_handle() as HANDLE) == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { _handle: handle })
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Reaction currently requires Windows",
            ))
        }
    }
}
