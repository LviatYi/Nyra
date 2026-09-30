use std::{fmt, path::PathBuf};

use bevy::prelude::Resource;
use serde::{Deserialize, Deserializer, de};

/// Seconds.
pub type JobTick = u64;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tip {
    pub text: String,
    pub color: Option<String>,
    pub reaction: Option<TipReaction>,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Job {
    Interval {
        tip: Tip,
        interval: JobTick,
        show_time: Option<JobTick>,
    },
    ImagePerception {
        tip: Tip,
        image: PathBuf,
    },
}

impl Job {
    pub fn tip(&self) -> &Tip {
        match self {
            Self::Interval { tip, .. } | Self::ImagePerception { tip, .. } => tip,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TipReaction {
    pub hotkey: Hotkey,
    pub script: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub(crate) modifiers: u32,
    pub(crate) virtual_key: u32,
    display: String,
}

impl Hotkey {
    fn parse(value: &str) -> Result<Self, String> {
        const MOD_ALT: u32 = 0x0001;
        const MOD_CONTROL: u32 = 0x0002;
        const MOD_SHIFT: u32 = 0x0004;
        const MOD_WIN: u32 = 0x0008;

        let parts = value.split('+').map(str::trim).collect::<Vec<_>>();
        if parts.iter().any(|part| part.is_empty()) || parts.len() > 5 {
            return Err("Hotkey format should be like Ctrl+Alt+R or F8".into());
        }
        let Some((key, modifiers)) = parts.split_last() else {
            return Err("Hotkey cannot be empty".into());
        };
        let mut modifier_bits = 0;
        for modifier in modifiers {
            let bit = match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => MOD_CONTROL,
                "alt" => MOD_ALT,
                "shift" => MOD_SHIFT,
                "win" | "meta" => MOD_WIN,
                _ => return Err(format!("Unsupported hotkey modifier: {modifier}")),
            };
            if modifier_bits & bit != 0 {
                return Err(format!("Duplicate hotkey modifier: {modifier}"));
            }
            modifier_bits |= bit;
        }

        let upper = key.to_ascii_uppercase();
        let (virtual_key, key_name, plain_key_allowed) = match upper.as_str() {
            "SPACE" => (0x20, "Space".to_string(), false),
            "ENTER" => (0x0D, "Enter".to_string(), false),
            "ESC" | "ESCAPE" => (0x1B, "Escape".to_string(), false),
            "TAB" => (0x09, "Tab".to_string(), false),
            _ if upper.len() == 1 && upper.as_bytes()[0].is_ascii_alphanumeric() => {
                (upper.as_bytes()[0] as u32, upper, false)
            }
            _ if upper.starts_with('F') => {
                let number = upper[1..]
                    .parse::<u32>()
                    .map_err(|_| format!("Unsupported hotkey key: {key}"))?;
                if !(1..=24).contains(&number) {
                    return Err(format!("Function keys must be in the range F1..F24: {key}"));
                }
                (0x70 + number - 1, format!("F{number}"), true)
            }
            _ => return Err(format!("Unsupported hotkey key: {key}")),
        };
        if modifier_bits == 0 && !plain_key_allowed {
            return Err(
                "Letters, numbers, and ordinary keys must have at least one modifier key".into(),
            );
        }

        let mut display_parts = Vec::new();
        if modifier_bits & MOD_CONTROL != 0 {
            display_parts.push("Ctrl");
        }
        if modifier_bits & MOD_ALT != 0 {
            display_parts.push("Alt");
        }
        if modifier_bits & MOD_SHIFT != 0 {
            display_parts.push("Shift");
        }
        if modifier_bits & MOD_WIN != 0 {
            display_parts.push("Win");
        }
        display_parts.push(&key_name);
        Ok(Self {
            modifiers: modifier_bits,
            virtual_key,
            display: display_parts.join("+"),
        })
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for Hotkey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

pub fn is_valid_tip_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Deserialize)]
pub struct Jobs {
    pub jobs: Vec<Job>,
}

#[derive(Resource)]
pub struct JobConfig(pub Jobs);

impl JobConfig {
    pub fn is_empty(&self) -> bool {
        self.0.jobs.is_empty()
    }

    /// Returns the configured color or the palette color assigned to this job position.
    pub fn resolved_color_at(&self, job_index: usize) -> &str {
        self.0.jobs[job_index]
            .tip()
            .color
            .as_deref()
            .unwrap_or_else(|| {
                let colors = crate::settings_default_values::DEFAULT_TIP_COLORS;
                colors[job_index % colors.len()]
            })
    }
}
