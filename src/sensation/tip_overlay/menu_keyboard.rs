use std::{
    ptr::{null, null_mut},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

use bevy::prelude::Resource;
use windows_sys::Win32::{
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::{
        Input::{
            KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE},
            RAWINPUTDEVICE, RIDEV_REMOVE, RegisterRawInputDevices,
        },
        WindowsAndMessaging::{
            CallNextHookEx, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE,
            PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
            WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};

struct HookState {
    keys: Vec<u32>,
    active: AtomicBool,
    captured: Vec<AtomicBool>,
    ignored_until_release: Vec<AtomicBool>,
    triggered: Vec<AtomicBool>,
}

static HOOK_STATE: OnceLock<Arc<HookState>> = OnceLock::new();

#[derive(Resource)]
pub(crate) struct MenuKeyboard {
    state: Arc<HookState>,
    dismiss_key_index: usize,
    thread_id: u32,
    worker: Option<JoinHandle<()>>,
}

impl MenuKeyboard {
    pub(crate) fn new(mut keys: Vec<u32>) -> Result<Self, String> {
        // Initialize after winit, which registers both raw keyboard and mouse input.
        // Keep keyboard input on the low-level hook and legacy window messages.
        let keyboard = RAWINPUTDEVICE {
            usUsagePage: 0x01, // Generic Desktop Controls.
            usUsage: 0x06,     // Keyboard; leave raw mouse input registered.
            dwFlags: RIDEV_REMOVE,
            hwndTarget: null_mut(),
        };
        if unsafe { RegisterRawInputDevices(&keyboard, 1, size_of::<RAWINPUTDEVICE>() as u32) } == 0
        {
            return Err(format!(
                "Unable to unregister raw keyboard input: {}",
                std::io::Error::last_os_error()
            ));
        }

        let dismiss_key_index = keys.len();
        keys.push(VK_ESCAPE as u32);
        let key_count = keys.len();
        let flags = || (0..key_count).map(|_| AtomicBool::new(false)).collect();
        let state = Arc::new(HookState {
            keys,
            active: AtomicBool::new(false),
            captured: flags(),
            ignored_until_release: flags(),
            triggered: flags(),
        });
        HOOK_STATE
            .set(state.clone())
            .map_err(|_| "Menu keyboard hook is already initialized".to_string())?;

        let (startup_tx, startup_rx) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("menu-keyboard".into())
            .spawn(move || hook_thread(startup_tx))
            .map_err(|error| format!("Unable to start menu keyboard thread: {error}"))?;
        let thread_id = startup_rx
            .recv()
            .map_err(|_| "Menu keyboard thread exited during initialization".to_string())??;
        Ok(Self {
            state,
            dismiss_key_index,
            thread_id,
            worker: Some(worker),
        })
    }

    pub(super) fn set_active(&self, active: bool) {
        if self.state.active.load(Ordering::SeqCst) == active {
            return;
        }
        if active {
            for (index, key) in self.state.keys.iter().enumerate() {
                let down = unsafe { GetAsyncKeyState(*key as i32) as u16 & 0x8000 != 0 };
                self.state.ignored_until_release[index].store(
                    down && !self.state.captured[index].load(Ordering::SeqCst),
                    Ordering::SeqCst,
                );
            }
        }
        self.state.active.store(active, Ordering::SeqCst);
    }

    pub(super) fn take_triggered(&self, index: usize) -> bool {
        self.state.triggered[index].swap(false, Ordering::SeqCst)
    }

    pub(super) fn take_dismissed(&self) -> bool {
        self.take_triggered(self.dismiss_key_index)
    }
}

impl Drop for MenuKeyboard {
    fn drop(&mut self) {
        self.state.active.store(false, Ordering::SeqCst);
        if unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) } != 0 {
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

fn hook_thread(startup: mpsc::SyncSender<Result<u32, String>>) {
    let thread_id = unsafe { GetCurrentThreadId() };
    let mut message = MSG::default();
    unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE) };

    let module = unsafe { GetModuleHandleW(null()) };
    if module.is_null() {
        let _ = startup.send(Err(format!(
            "Unable to get menu keyboard module: {}",
            std::io::Error::last_os_error()
        )));
        return;
    }
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0) };
    if hook.is_null() {
        let _ = startup.send(Err(format!(
            "Unable to install menu keyboard hook: {}",
            std::io::Error::last_os_error()
        )));
        return;
    }
    if startup.send(Ok(thread_id)).is_err() {
        unsafe { UnhookWindowsHookEx(hook) };
        return;
    }

    while unsafe { GetMessageW(&mut message, null_mut(), 0, 0) } > 0 {}
    unsafe { UnhookWindowsHookEx(hook) };
}

unsafe extern "system" fn keyboard_hook(code: i32, message: usize, data: isize) -> isize {
    if code == HC_ACTION as i32
        && let Some(state) = HOOK_STATE.get()
    {
        let key = unsafe { (*(data as *const KBDLLHOOKSTRUCT)).vkCode };
        if let Some(index) = state.keys.iter().position(|candidate| *candidate == key) {
            let key_down = message == WM_KEYDOWN as usize || message == WM_SYSKEYDOWN as usize;
            let key_up = message == WM_KEYUP as usize || message == WM_SYSKEYUP as usize;
            if state.ignored_until_release[index].load(Ordering::SeqCst) {
                if key_up {
                    state.ignored_until_release[index].store(false, Ordering::SeqCst);
                }
            } else if state.active.load(Ordering::SeqCst)
                || state.captured[index].load(Ordering::SeqCst)
            {
                if key_down {
                    if !state.captured[index].swap(true, Ordering::SeqCst) {
                        state.triggered[index].store(true, Ordering::SeqCst);
                    }
                    return 1;
                }
                if key_up && state.captured[index].swap(false, Ordering::SeqCst) {
                    return 1;
                }
            }
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, message, data) }
}
