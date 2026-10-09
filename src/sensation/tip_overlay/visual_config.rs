use bevy::{
    prelude::*,
    sprite::{BorderRect, SpriteImageMode, TextureSlicer},
    ui::widget::NodeImageMode,
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
    pub countdown_background: Option<LayerAsset>,
    pub border_path: Option<String>,
    pub border_width: f32,
    pub background_opacity: f32,
}

impl Default for RotationNoticeVisualConfig {
    fn default() -> Self {
        Self {
            base_background: None,
            icon_background: None,
            countdown_background: None,
            border_path: Some("sensation/rotation_notice_border.json".into()),
            border_width: 2.0,
            background_opacity: 0.72,
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
    asset_server: &AssetServer,
    asset: Option<&LayerAsset>,
    size: Vec2,
) -> Sprite {
    let Some(asset) = asset.filter(|asset| !asset.path.trim().is_empty()) else {
        return Sprite::from_color(Color::NONE, size);
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
    Sprite {
        image: asset_server.load(asset.path.clone()),
        image_mode,
        custom_size,
        ..default()
    }
}

pub(super) fn layer_image(asset_server: &AssetServer, asset: Option<&LayerAsset>) -> ImageNode {
    let Some(asset) = asset.filter(|asset| !asset.path.trim().is_empty()) else {
        return ImageNode::default();
    };
    let mode = match asset.sizing {
        LayerSizing::Original => NodeImageMode::Auto,
        LayerSizing::Stretch => NodeImageMode::Stretch,
        LayerSizing::Sliced { border } => NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(border.max(0.0)),
            ..default()
        }),
    };
    ImageNode::new(asset_server.load(asset.path.clone())).with_mode(mode)
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
