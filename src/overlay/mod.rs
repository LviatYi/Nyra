use crate::overlay::selection::SelectionPhase;

pub mod asset;
pub mod overlay_app;
mod selection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMode {
    Edit,
    Sentinel,
}

impl OverlayMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Edit => Self::Sentinel,
            Self::Sentinel => Self::Edit,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OverlayState {
    pub mode: OverlayMode,
    pub selection: SelectionPhase,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            mode: OverlayMode::Edit,
            selection: SelectionPhase::default(),
        }
    }
}
