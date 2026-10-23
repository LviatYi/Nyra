use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A physical pixel coordinate in the Windows virtual screen coordinate space.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenPoint {
    /// Horizontal physical pixel coordinate.
    pub x: i32,
    /// Vertical physical pixel coordinate.
    pub y: i32,
}

#[derive(Deserialize, Serialize, TS)]
pub enum DelayConditionKind {
    #[serde(rename = "delay")]
    Delay,
}

/// A fixed blocking delay, usable before or after an input action.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DelayCondition {
    #[ts(inline)]
    pub kind: DelayConditionKind,
    pub duration_ms: u64,
}

#[derive(Deserialize, Serialize, TS)]
pub enum ImageConditionKind {
    #[serde(rename = "image")]
    Image,
}

/// A Perception JSON resource that must match before an input action starts.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageCondition {
    #[ts(inline)]
    pub kind: ImageConditionKind,
    pub path: String,
    #[serde(default = "default_condition_timeout_ms")]
    #[ts(as = "Option<u64>", optional)]
    pub timeout_ms: u64,
    #[serde(default = "default_condition_poll_interval_ms")]
    #[ts(as = "Option<u64>", optional)]
    pub poll_interval_ms: u64,
}

/// A blocking prerequisite for an input action.
#[derive(Deserialize, Serialize, TS)]
#[serde(untagged)]
pub enum UiCondition {
    Delay(DelayCondition),
    Image(ImageCondition),
}

/// A side action performed after successful input.
#[derive(Deserialize, Serialize, TS)]
#[serde(untagged)]
pub enum AfterAction {
    Delay(DelayCondition),
}

/// An input action independent of its prerequisites and attached actions.
#[derive(Deserialize, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InputAction {
    Click(ClickInput),
    Drag(DragInput),
}

/// Click payload; the enclosing InputAction supplies the kind discriminator.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClickInput {
    pub point: ScreenPoint,
    pub repeat: Option<ClickRepeatOption>,
}

/// Options for repeating a click action.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClickRepeatOption {
    pub count: u32,
    pub interval_ms: u64,
}

impl Default for ClickRepeatOption {
    fn default() -> Self {
        Self {
            count: 1,
            interval_ms: 50,
        }
    }
}

/// Drag payload; the enclosing InputAction supplies the kind discriminator.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DragInput {
    pub from: ScreenPoint,
    pub to: ScreenPoint,
    pub duration_ms: u64,
}

/// A complete synchronous operation, shared by native and debug backends.
#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionStep {
    /// A condition that must complete before the action starts.
    pub before: Option<UiCondition>,
    /// The input action to perform.
    pub action: InputAction,
    /// An attached action performed only after successful input.
    pub after: Option<AfterAction>,
    /// Optional description included in the operation log.
    pub message: Option<String>,
}

fn default_condition_timeout_ms() -> u64 {
    3_000
}

fn default_condition_poll_interval_ms() -> u64 {
    50
}
