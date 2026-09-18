use std::{
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
};

use bevy::prelude::Resource;

use crate::job::Hotkey;

const CHANGE_HOTKEY_MESSAGE: u32 = 0x8000 + 1;
const HOTKEY_ID: i32 = 1;

pub(super) enum HotkeyEvent {
    Registered(Option<Hotkey>),
    Triggered(Hotkey),
    RegistrationFailed(String),
}

#[derive(Resource)]
pub(crate) struct GlobalHotkeys {
    requested: Option<Hotkey>,
    command: Arc<Mutex<Option<Option<Hotkey>>>>,
    events: Mutex<mpsc::Receiver<HotkeyEvent>>,
    thread_id: u32,
    worker: Option<JoinHandle<()>>,
}

impl GlobalHotkeys {
    pub(crate) fn new() -> Result<Self, String> {
        let command = Arc::new(Mutex::new(None));
        let worker_command = command.clone();
        let (event_tx, event_rx) = mpsc::channel();
        let (startup_tx, startup_rx) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("reaction-hotkey".into())
            .spawn(move || hotkey_thread(worker_command, event_tx, startup_tx))
            .map_err(|error| format!("Unable to start global hotkey thread: {error}"))?;
        let thread_id = startup_rx
            .recv()
            .map_err(|_| "Global hotkey thread exited during initialization".to_string())??;
        Ok(Self {
            requested: None,
            command,
            events: Mutex::new(event_rx),
            thread_id,
            worker: Some(worker),
        })
    }

    pub(super) fn set_active(&mut self, hotkey: Option<Hotkey>) -> Result<(), String> {
        if self.requested == hotkey {
            return Ok(());
        }
        *self.command.lock().unwrap() = Some(hotkey.clone());
        if !post_thread_message(self.thread_id, CHANGE_HOTKEY_MESSAGE) {
            self.command.lock().unwrap().take();
            return Err(format!(
                "Unable to update global hotkey: {}",
                std::io::Error::last_os_error()
            ));
        }
        self.requested = hotkey;
        Ok(())
    }

    pub(super) fn drain(&self) -> Vec<HotkeyEvent> {
        let receiver = self.events.lock().unwrap();
        std::iter::from_fn(|| receiver.try_recv().ok()).collect()
    }
}

impl Drop for GlobalHotkeys {
    fn drop(&mut self) {
        if post_thread_message(self.thread_id, windows_quit_message()) {
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

#[cfg(windows)]
fn hotkey_thread(
    command: Arc<Mutex<Option<Option<Hotkey>>>>,
    events: mpsc::Sender<HotkeyEvent>,
    startup: mpsc::SyncSender<Result<u32, String>>,
) {
    use std::ptr::null_mut;

    use windows_sys::Win32::{
        System::Threading::GetCurrentThreadId,
        UI::{
            Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey},
            WindowsAndMessaging::{GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, WM_HOTKEY},
        },
    };

    let thread_id = unsafe { GetCurrentThreadId() };
    let mut message = MSG::default();
    unsafe {
        // Create this thread's message queue before another thread posts commands.
        PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
    }
    if startup.send(Ok(thread_id)).is_err() {
        return;
    }

    let mut registered = None::<Hotkey>;
    loop {
        let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        if message.message == CHANGE_HOTKEY_MESSAGE {
            let Some(next) = command.lock().unwrap().take() else {
                continue;
            };
            if registered.take().is_some() {
                unsafe { UnregisterHotKey(null_mut(), HOTKEY_ID) };
            }
            if let Some(hotkey) = next {
                let registered_ok = unsafe {
                    RegisterHotKey(
                        null_mut(),
                        HOTKEY_ID,
                        hotkey.modifiers | MOD_NOREPEAT,
                        hotkey.virtual_key,
                    )
                } != 0;
                if registered_ok {
                    registered = Some(hotkey.clone());
                    let _ = events.send(HotkeyEvent::Registered(Some(hotkey)));
                } else {
                    let _ = events.send(HotkeyEvent::RegistrationFailed(format!(
                        "Unable to register global hotkey {hotkey}: {}",
                        std::io::Error::last_os_error()
                    )));
                }
            } else {
                let _ = events.send(HotkeyEvent::Registered(None));
            }
        } else if message.message == WM_HOTKEY && message.wParam == HOTKEY_ID as usize {
            if let Some(hotkey) = registered.clone() {
                let _ = events.send(HotkeyEvent::Triggered(hotkey));
            }
        }
    }

    if registered.is_some() {
        unsafe { UnregisterHotKey(null_mut(), HOTKEY_ID) };
    }
}

#[cfg(not(windows))]
fn hotkey_thread(
    _command: Arc<Mutex<Option<Option<Hotkey>>>>,
    _events: mpsc::Sender<HotkeyEvent>,
    startup: mpsc::SyncSender<Result<u32, String>>,
) {
    let _ = startup.send(Err(
        "Global hotkeys are currently only supported on Windows".into(),
    ));
}

#[cfg(windows)]
fn post_thread_message(thread_id: u32, message: u32) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

    unsafe { PostThreadMessageW(thread_id, message, 0, 0) != 0 }
}

#[cfg(not(windows))]
fn post_thread_message(_thread_id: u32, _message: u32) -> bool {
    false
}

#[cfg(windows)]
fn windows_quit_message() -> u32 {
    windows_sys::Win32::UI::WindowsAndMessaging::WM_QUIT
}

#[cfg(not(windows))]
fn windows_quit_message() -> u32 {
    0
}
