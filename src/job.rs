use bevy::prelude::Resource;
use serde::Deserialize;

/// Seconds.
pub type TipTick = u64;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tip {
    pub tip: String,
    pub interval: TipTick,
    pub show_time: Option<TipTick>,
    pub color: Option<String>,
}

impl Tip {
    pub fn show_time(&self) -> TipTick {
        self.show_time
            .unwrap_or(crate::settings_default_values::DEFAULT_TIP_SHOW_TIME)
    }
}

pub fn is_valid_tip_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Deserialize)]
pub struct Tips {
    pub tips: Vec<Tip>,
}

#[derive(Resource)]
pub struct JobConfig(pub Tips);

impl JobConfig {
    pub fn is_empty(&self) -> bool {
        self.0.tips.is_empty()
    }

    /// Returns the configured color or the palette color assigned to this tip position.
    pub fn resolved_color_at(&self, tip_index: usize) -> &str {
        self.0.tips[tip_index].color.as_deref().unwrap_or_else(|| {
            let colors = crate::settings_default_values::DEFAULT_TIP_COLORS;
            colors[tip_index % colors.len()]
        })
    }
}