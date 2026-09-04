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
}

impl Tip {
    pub fn show_time(&self) -> TipTick {
        self.show_time
            .unwrap_or(crate::settings_default_values::DEFAULT_TIP_SHOW_TIME)
    }
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
}
