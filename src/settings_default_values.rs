// some default values will be moved to the settings in the future.

use crate::job::JobTick;
use std::time::Duration;

//region Windows
// The sensation background is authored at 4x (800x200).
pub const WINDOW_WIDTH: u32 = 800 / 4;
pub const WINDOW_HEIGHT: u32 = 200 / 4;
//endregion

//region UI
pub const DEFAULT_TIP_SHOW_TIME: JobTick = 5u64;
pub const DEFAULT_TIP_COLORS: &[&str] = &[
    "#33E078", "#4DA3FF", "#A879FF", "#FFB347", "#FF6685", "#37D6D0",
];

pub const TIP_TEXT_SCROLL_START_WAIT_DURATION: Duration = Duration::from_millis(1500);
pub const TIP_TEXT_SCROLL_END_WAIT_DURATION: Duration = Duration::from_millis(1000);
pub const TIP_TEXT_SCROLL_SPEED: f32 = 120.0;
pub const TIP_TEXT_SCROLL_ACCELERATION: f32 = 240.0;
pub const TIP_TEXT_SCROLL_RETURN_SPEED: f32 = 8000.0;
pub const TIP_TEXT_SCROLL_RETURN_DECELERATION: f32 = 24000.0;
//endregion
