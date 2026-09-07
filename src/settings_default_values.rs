// some default values will be moved to the settings in the future.

use crate::job::TipTick;

//region Windows
pub const WINDOW_WIDTH: u32 = 160;
pub const WINDOW_HEIGHT: u32 = 40;
//endregion

//region UI
pub const DEFAULT_TIP_SHOW_TIME: TipTick = 5u64;
pub const DEFAULT_TIP_COLORS: &[&str] = &[
    "#33E078", "#4DA3FF", "#A879FF", "#FFB347", "#FF6685", "#37D6D0",
];

pub const TIP_OVERLAY_BORDER_WIDTH: f32 = 5.0;
pub const TIP_OVERLAY_CORNER_RADIUS: f32 = 12.0;
pub const TIP_OVERLAY_CORNER_SEGMENTS: usize = 12;
pub const TIP_OVERLAY_BACKGROUND_BRIGHTNESS: f32 = 0.18;
pub const TIP_OVERLAY_BACKGROUND_ALPHA: f32 = 0.60;
//endregion
