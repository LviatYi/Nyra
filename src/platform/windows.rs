use crate::overlay::OverlayMode;
use crate::platform::OverlayPlatformEvent;
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, EnumWindows, GetWindowLongW, GetWindowTextW,
    GetWindowThreadProcessId, GWLP_WNDPROC, GWL_EXSTYLE, HWND_TOPMOST, IsWindowVisible,
    LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowLongW, SetWindowPos, WINDOW_EX_STYLE,
    WM_HOTKEY, WNDPROC, WS_EX_LAYERED, WS_EX_TRANSPARENT,
};
use windows::core::BOOL;

const HOTKEY_ID_TOGGLE_MODE: i32 = 1;
const HOTKEY_ID_CLEAR_SELECTION: i32 = 2;
const MOD_NOREPEAT: u32 = 0x4000;
const VK_ESCAPE: u32 = 0x1B;
const VK_F8: u32 = 0x77;

static SUBCLASS_STATE: OnceLock<Mutex<OverlayWindowSubclassState>> = OnceLock::new();

#[derive(Default)]
pub struct WindowsOverlayController {
    title: String,
    hwnd: Option<HWND>,
    last_mode: Option<OverlayMode>,
    hotkeys_registered: bool,
    subclass_installed: bool,
}

#[derive(Default)]
struct OverlayWindowSubclassState {
    hwnd: Option<isize>,
    original_wndproc: WNDPROC,
    queued_events: Vec<OverlayPlatformEvent>,
}

impl WindowsOverlayController {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            hwnd: None,
            last_mode: None,
            hotkeys_registered: false,
            subclass_installed: false,
        }
    }

    pub fn apply_mode(&mut self, mode: OverlayMode) {
        if self.last_mode == Some(mode) {
            return;
        }

        let hwnd = match self.resolve_hwnd() {
            Some(hwnd) => hwnd,
            None => return,
        };

        self.bind_window(hwnd);

        unsafe {
            let current = WINDOW_EX_STYLE(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32);
            let mut next = current;
            next |= WS_EX_LAYERED;

            match mode {
                OverlayMode::Edit => next &= !WS_EX_TRANSPARENT,
                OverlayMode::Sentinel => next |= WS_EX_TRANSPARENT,
            }

            if next != current {
                let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, next.0 as i32);
            }

            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_NOOWNERZORDER | SWP_NOZORDER,
            );
        }

        self.last_mode = Some(mode);
    }

    pub fn poll_events(&mut self) -> Vec<OverlayPlatformEvent> {
        let state = subclass_state();
        let mut state = state.lock().expect("overlay subclass state poisoned");
        let mut events = Vec::new();
        std::mem::swap(&mut events, &mut state.queued_events);
        events
    }

    fn bind_window(&mut self, hwnd: HWND) {
        if !self.subclass_installed {
            self.install_subclass(hwnd);
        }

        if !self.hotkeys_registered {
            self.register_hotkeys(hwnd);
        }
    }

    fn install_subclass(&mut self, hwnd: HWND) {
        unsafe {
            let previous =
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, overlay_window_proc as usize as isize);
            let state = subclass_state();
            let mut state = state.lock().expect("overlay subclass state poisoned");
            state.hwnd = Some(hwnd.0 as isize);
            state.original_wndproc = std::mem::transmute(previous);
        }

        self.subclass_installed = true;
    }

    fn register_hotkeys(&mut self, hwnd: HWND) {
        unsafe {
            let toggle_ok = RegisterHotKey(
                Some(hwnd),
                HOTKEY_ID_TOGGLE_MODE,
                HOT_KEY_MODIFIERS(MOD_NOREPEAT),
                VK_F8,
            );
            let clear_ok = RegisterHotKey(
                Some(hwnd),
                HOTKEY_ID_CLEAR_SELECTION,
                HOT_KEY_MODIFIERS(MOD_NOREPEAT),
                VK_ESCAPE,
            );
            self.hotkeys_registered = toggle_ok.is_ok() && clear_ok.is_ok();
        }
    }

    fn unregister_hotkeys(&mut self) {
        if !self.hotkeys_registered {
            return;
        }

        if let Some(hwnd) = self.hwnd {
            unsafe {
                let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID_TOGGLE_MODE);
                let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID_CLEAR_SELECTION);
            }
        }

        self.hotkeys_registered = false;
    }

    fn restore_subclass(&mut self) {
        if !self.subclass_installed {
            return;
        }

        let Some(hwnd) = self.hwnd else {
            return;
        };

        let state = subclass_state();
        let mut state = state.lock().expect("overlay subclass state poisoned");
        if state.hwnd == Some(hwnd.0 as isize) {
            if let Some(original_wndproc) = state.original_wndproc {
                unsafe {
                    let _ = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, original_wndproc as usize as isize);
                }
            }
            *state = OverlayWindowSubclassState::default();
        }

        self.subclass_installed = false;
    }

    fn resolve_hwnd(&mut self) -> Option<HWND> {
        if self.hwnd.is_some() {
            return self.hwnd;
        }

        let mut search = WindowSearch {
            process_id: unsafe { GetCurrentProcessId() },
            title: self.title.clone(),
            found: None,
        };

        unsafe {
            let _ = EnumWindows(
                Some(enum_windows_proc),
                LPARAM((&mut search as *mut WindowSearch).cast::<()>() as isize),
            );
        }

        self.hwnd = search.found;
        self.hwnd
    }
}

impl Drop for WindowsOverlayController {
    fn drop(&mut self) {
        self.unregister_hotkeys();
        self.restore_subclass();
    }
}

fn subclass_state() -> &'static Mutex<OverlayWindowSubclassState> {
    SUBCLASS_STATE.get_or_init(|| Mutex::new(OverlayWindowSubclassState::default()))
}

unsafe extern "system" fn overlay_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_HOTKEY {
        let state = subclass_state();
        let mut state = state.lock().expect("overlay subclass state poisoned");
        if state.hwnd == Some(hwnd.0 as isize) {
            match wparam.0 as i32 {
                HOTKEY_ID_TOGGLE_MODE => state.queued_events.push(OverlayPlatformEvent::ToggleMode),
                HOTKEY_ID_CLEAR_SELECTION => {
                    state.queued_events.push(OverlayPlatformEvent::ClearSelection)
                }
                _ => {}
            }
            return LRESULT(0);
        }
    }

    let original_wndproc = {
        let state = subclass_state();
        let state = state.lock().expect("overlay subclass state poisoned");
        if state.hwnd == Some(hwnd.0 as isize) {
            state.original_wndproc
        } else {
            None
        }
    };

    match original_wndproc {
        Some(original_wndproc) => unsafe {
            CallWindowProcW(Some(original_wndproc), hwnd, msg, wparam, lparam)
        },
        None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

struct WindowSearch {
    process_id: u32,
    title: String,
    found: Option<HWND>,
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = unsafe { &mut *(lparam.0 as *mut WindowSearch) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL(1);
    }

    let mut process_id = 0;
    let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if process_id != search.process_id {
        return BOOL(1);
    }

    let mut title_buffer = [0u16; 256];
    let length = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
    if length <= 0 {
        return BOOL(1);
    }

    let current_title = String::from_utf16_lossy(&title_buffer[..length as usize]);
    if current_title == search.title {
        search.found = Some(hwnd);
        return BOOL(0);
    }

    BOOL(1)
}
