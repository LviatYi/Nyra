use bevy::{
    prelude::*,
    sprite::{BorderRect, SpriteImageMode, SpriteTemplate, TextureSlicer},
};
use serde::Deserialize;
use std::fs;

const VISUAL_CONFIG_PATH: &str = "assets/sensation/rotation_notice_visual.json";

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayerAsset {
    /// Path relative to Bevy's assets directory.
    pub path: String,
    #[serde(default)]
    pub tint_with_theme: bool,
    #[serde(default = "opaque")]
    pub opacity: f32,
    #[serde(default)]
    pub sizing: LayerSizing,
}

fn opaque() -> f32 {
    1.0
}

#[derive(Clone, Default, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum LayerSizing {
    #[default]
    Original,
    Stretch,
    Sliced {
        border: f32,
    },
}

#[derive(Resource, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RotationNoticeVisualConfig {
    pub base_background: Option<LayerAsset>,
    pub icon_background: Option<LayerAsset>,
    pub border_path: Option<String>,
    pub border_width: f32,
    pub background_opacity: f32,
}

impl Default for RotationNoticeVisualConfig {
    fn default() -> Self {
        Self {
            base_background: Some(LayerAsset {
                path: "sensation/sensation-layer-background@4x.png".into(),
                tint_with_theme: true,
                opacity: 1.0,
                sizing: LayerSizing::Stretch,
            }),
            icon_background: None,
            border_path: Some("sensation/rotation_notice_border.json".into()),
            border_width: 1.0,
            background_opacity: 1.0,
        }
    }
}

impl RotationNoticeVisualConfig {
    pub(super) fn load() -> Self {
        let source = match fs::read_to_string(VISUAL_CONFIG_PATH) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("[Sensation] {VISUAL_CONFIG_PATH}: {error}; using built-in visuals");
                return Self::default();
            }
        };
        serde_json::from_str::<Self>(&source).unwrap_or_else(|error| {
            eprintln!("[Sensation] {VISUAL_CONFIG_PATH}: {error}; using built-in visuals");
            Self::default()
        })
    }
}

pub(super) fn layer_sprite(
    asset: Option<&LayerAsset>,
    size: Vec2,
) -> SpriteTemplate {
    let Some(asset) = asset.filter(|asset| !asset.path.trim().is_empty()) else {
        return SpriteTemplate {
            custom_size: Some(size),
            ..default()
        };
    };
    let (image_mode, custom_size) = match asset.sizing {
        LayerSizing::Original => (SpriteImageMode::Auto, None),
        LayerSizing::Stretch => (SpriteImageMode::Auto, Some(size)),
        LayerSizing::Sliced { border } => (
            SpriteImageMode::Sliced(TextureSlicer {
                border: BorderRect::all(border.max(0.0)),
                ..default()
            }),
            Some(size),
        ),
    };
    SpriteTemplate {
        image: asset.path.clone().into(),
        image_mode,
        custom_size,
        ..default()
    }
}

pub(super) fn layer_color(asset: Option<&LayerAsset>, theme: Color, global_opacity: f32) -> Color {
    let Some(asset) = asset.filter(|asset| !asset.path.trim().is_empty()) else {
        return Color::NONE;
    };
    let opacity = (asset.opacity * global_opacity).clamp(0.0, 1.0);
    if asset.tint_with_theme {
        let theme = theme.to_srgba();
        Color::srgba(theme.red, theme.green, theme.blue, opacity)
    } else {
        Color::srgba(1.0, 1.0, 1.0, opacity)
    }
}
