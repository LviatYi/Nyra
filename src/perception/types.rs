use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Deserialize, Serialize, TS)]
pub enum ImageConditionKind {
    #[serde(rename = "image")]
    Image,
}

/// A fixed region measured in physical pixels from the monitor's top-left corner.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PositionUnit {
    #[default]
    Pixels,
    Ratio,
}

/// Position relative to the window's top-left corner; dimensions always remain physical pixels.
#[derive(Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowRegion {
    pub x: f64,
    pub y: f64,
    pub width: u32,
    pub height: u32,
    /// Defaults to pixels. Ratio positions multiply x/y by the current window width/height.
    #[serde(default)]
    #[ts(as = "Option<PositionUnit>", optional)]
    pub unit: PositionUnit,
}

#[derive(Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonitorConfig {
    /// Omitted or null selects the user's primary monitor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
}

/// A screen region or a window selected by executable filename first, then exact window title.
#[derive(Debug, Deserialize, Serialize, TS)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CaptureTarget {
    Screen {
        monitor: MonitorConfig,
        region: Region,
    },
    Window {
        /// Matches executable filenames (case-insensitive) before exact window titles.
        title: String,
        region: WindowRegion,
    },
}

/// An image match configured directly in TypeScript, used before an input action.
#[derive(Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageCondition {
    #[ts(inline)]
    pub kind: ImageConditionKind,
    pub target: CaptureTarget,
    /// Template image path, relative to the main configuration directory unless absolute.
    pub template: String,
    /// Maximum position deviation in physical pixels on each axis; defaults to 16.
    #[serde(default = "default_offset_tolerance")]
    #[ts(as = "Option<u32>", optional)]
    pub offset_tolerance: u32,
    /// Minimum score in [0, 1]: 1 minus mean squared RGB difference divided by 255²; defaults to 0.95.
    #[serde(default = "default_condition_threshold")]
    #[ts(as = "Option<f64>", optional)]
    pub threshold: f64,
    #[serde(default = "default_condition_timeout_ms")]
    #[ts(as = "Option<u64>", optional)]
    pub timeout_ms: u64,
    #[serde(default = "default_condition_poll_interval_ms")]
    #[ts(as = "Option<u64>", optional)]
    pub poll_interval_ms: u64,
}

fn default_condition_timeout_ms() -> u64 {
    3_000
}
fn default_condition_threshold() -> f64 {
    0.95
}
fn default_condition_poll_interval_ms() -> u64 {
    50
}
fn default_offset_tolerance() -> u32 {
    16
}
