use bevy::prelude::Resource;
use serde::Deserialize;

pub type TipTick = f32;

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
