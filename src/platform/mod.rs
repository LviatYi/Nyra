#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::WindowsOverlayController as OverlayController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPlatformEvent {
    ToggleMode,
    ClearSelection,
}

#[cfg(not(target_os = "windows"))]
#[derive(Default)]
pub struct OverlayController;

#[cfg(not(target_os = "windows"))]
impl OverlayController {
    pub fn new(_: &str) -> Self {
        Self
    }

    pub fn apply_mode(&mut self, _: crate::overlay::OverlayMode) {}

    pub fn poll_events(&mut self) -> Vec<OverlayPlatformEvent> {
        Vec::new()
    }
}
