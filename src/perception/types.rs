use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
pub enum ImageConditionKind {
    #[serde(rename = "image")]
    Image,
}

/// A region measured in physical pixels from the capture target's top-left corner.
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

/// Position of the image anchor relative to the capture target.
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImagePosition {
    pub x: f64,
    pub y: f64,
    /// Defaults to physical pixels; ratio positions are relative to the capture target.
    #[serde(default)]
    #[ts(as = "Option<PositionUnit>", optional)]
    pub unit: PositionUnit,
    /// Maximum deviation from this position in physical pixels, default 16.
    #[serde(default = "default_offset_tolerance")]
    #[ts(as = "Option<u32>", optional)]
    pub tolerance: u32,
}

/// Anchor inside the template, from (0, 0) at the top left to (1, 1) at the bottom right.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageAnchor {
    pub x: f64,
    pub y: f64,
}

/// Restrict the search to an anchor position or a rectangle; omit to search the whole target.
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ImageSearch {
    Position { position: ImagePosition },
    Region { region: Region },
}

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MonitorConfig {
    /// Omitted or null selects the user's primary monitor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
}

/// A display or a window selected by executable filename or exact title.
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CaptureTarget {
    /// Select a monitor; null or omitted uses the primary monitor.
    FullScreen {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        monitor_name: Option<String>,
    },
    Screen {
        monitor: MonitorConfig,
    },
    Window {
        /// Matches executable filenames (case-insensitive) before exact window titles.
        title: String,
    },
}

impl Default for CaptureTarget {
    fn default() -> Self {
        Self::FullScreen { monitor_name: None }
    }
}

/// An image match configured directly in TypeScript, used before an input action.
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageCondition {
    #[ts(inline)]
    pub kind: ImageConditionKind,
    #[serde(default)]
    #[ts(as = "Option<CaptureTarget>", optional)]
    pub target: CaptureTarget,
    /// Template image path, relative to the main configuration directory unless absolute.
    pub template: String,
    /// Image anchor, defaulting to the top-left corner.
    #[serde(default)]
    #[ts(as = "Option<ImageAnchor>", optional)]
    pub anchor: ImageAnchor,
    /// Position or region to search; omitted searches the entire capture target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub search: Option<ImageSearch>,
    /// Enable unknown equal-proportion scale search. Disabled by default.
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub scale_invariant: bool,
    /// Minimum RGB similarity in [0, 1]. Defaults to 0.95 or 0.995 for scale search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub poll_interval_ms: Option<u64>,
}

impl ImageCondition {
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
            .unwrap_or(if self.scale_invariant { 15_000 } else { 3_000 })
    }

    pub fn poll_interval_ms(&self) -> u64 {
        self.poll_interval_ms
            .unwrap_or(if self.scale_invariant { 250 } else { 50 })
    }
}
fn default_offset_tolerance() -> u32 {
    16
}
