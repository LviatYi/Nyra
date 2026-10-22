use std::path::PathBuf;

use bevy::prelude::Resource;
use global_hotkey::hotkey::HotKey;
use serde::{Deserialize, Deserializer, de};

/// Seconds.
pub type JobTick = u64;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tip {
    pub text: String,
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
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
    #[serde(deserialize_with = "deserialize_hotkey")]
    pub hotkey: HotKey,
    pub script: PathBuf,
}

fn deserialize_hotkey<'de, D>(deserializer: D) -> Result<HotKey, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    let normalized = value
        .split('+')
        .map(str::trim)
        .map(|token| {
            if token.eq_ignore_ascii_case("win") {
                "Super"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join("+");
    normalized.parse().map_err(de::Error::custom)
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
