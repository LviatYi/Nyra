mod sensation_config;
pub mod tip_overlay;

use crate::json_config_loader::JsonConfigLoader;
use crate::sensation::sensation_config::{SensationConfig, load_sensation_config};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, RawHandleWrapper};
use raw_window_handle::RawWindowHandle;
use windows_sys::Win32::UI::WindowsAndMessaging::{RemovePropW, SetPropW};

pub struct SensoryPlugin;

impl Plugin for SensoryPlugin {
    fn build(&self, app: &mut App) {
        configure(app);
    }
}

fn configure(app: &mut App) {
    #[cfg(target_os = "windows")]
    app.add_observer(mark_overlay_window_non_rude)
        .add_observer(unmark_overlay_window_non_rude);

    app.init_asset::<SensationConfig>()
        .init_asset_loader::<JsonConfigLoader<SensationConfig>>()
        .add_systems(Startup, load_sensation_config);

    tip_overlay::configure(app);
}

#[cfg(target_os = "windows")]
const NON_RUDE_HWND: windows_sys::core::PCWSTR = windows_sys::core::w!("NonRudeHWND");

#[cfg(target_os = "windows")]
pub fn mark_overlay_window_non_rude(
    event: On<Insert<RawHandleWrapper>>,
    windows: Query<&RawHandleWrapper, With<PrimaryWindow>>,
) {
    let Ok(handle) = windows.get(event.entity) else {
        return;
    };
    let RawWindowHandle::Win32(raw) = handle.get_window_handle() else {
        panic!("Primary window has no Win32 handle");
    };
    let hwnd = raw.hwnd.get() as *mut _;

    // Windows must see this property before the full-screen-sized window is shown.
    if unsafe { SetPropW(hwnd, NON_RUDE_HWND, 1 as *mut _) } == 0 {
        panic!(
            "Could not set NonRudeHWND: {}",
            std::io::Error::last_os_error()
        );
    }
}

#[cfg(target_os = "windows")]
pub fn unmark_overlay_window_non_rude(
    event: On<Remove<RawHandleWrapper>>,
    windows: Query<&RawHandleWrapper, With<PrimaryWindow>>,
) {
    let Ok(handle) = windows.get(event.entity) else {
        return;
    };
    if let RawWindowHandle::Win32(raw) = handle.get_window_handle() {
        unsafe { RemovePropW(raw.hwnd.get() as *mut _, NON_RUDE_HWND) };
    }
}
