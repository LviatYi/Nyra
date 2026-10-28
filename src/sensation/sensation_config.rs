use bevy::{
    asset::LoadState,
    prelude::*,
    sprite::{BorderRect, SpriteImageMode, SpriteTemplate, TextureSlicer},
};
use serde::Deserialize;

const VISUAL_CONFIG_PATH: &str = "sensation/sensation_config.json";

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

#[derive(Asset, TypePath, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SensationConfig {
    pub base_background: Option<LayerAsset>,
    pub icon_background: Option<LayerAsset>,
    pub float_light: Option<LayerAsset>,
    pub border_path: Option<String>,
    pub border_width: f32,
}

impl Default for SensationConfig {
    fn default() -> Self {
        Self {
            base_background: Some(LayerAsset {
                path: "sensation/sensation-layer-background@4x.png".into(),
                tint_with_theme: true,
                opacity: 1.0,
                sizing: LayerSizing::Stretch,
            }),
            icon_background: None,
            float_light: Some(LayerAsset {
                path: "sensation/sensation-layer-float-light@4x.png".into(),
                tint_with_theme: true,
                opacity: 1.0,
                sizing: LayerSizing::Stretch,
            }),
            border_path: Some("sensation/sensation_tip_overlay_border.json".into()),
            border_width: 1.0,
        }
    }
}

#[derive(Resource)]
pub(in crate::sensation) struct SensationConfigHandle {
    handle: Handle<SensationConfig>,
    fallback: SensationConfig,
}

impl SensationConfigHandle {
    pub(in crate::sensation) fn load(asset_server: &AssetServer) -> Self {
        Self {
            handle: asset_server.load(VISUAL_CONFIG_PATH),
            fallback: SensationConfig::default(),
        }
    }

    pub(in crate::sensation) fn resolve<'a>(
        &'a self,
        assets: &'a Assets<SensationConfig>,
        asset_server: &AssetServer,
    ) -> Option<&'a SensationConfig> {
        assets.get(&self.handle).or_else(|| {
            matches!(
                asset_server.get_load_state(self.handle.id()),
                Some(LoadState::Failed(_))
            )
            .then_some(&self.fallback)
        })
    }
}

pub(in crate::sensation) fn load_sensation_config(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.insert_resource(SensationConfigHandle::load(&asset_server));
}

pub(in crate::sensation) fn layer_sprite(asset: Option<&LayerAsset>, size: Vec2) -> SpriteTemplate {
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

pub(in crate::sensation) fn layer_color(asset: Option<&LayerAsset>, theme: Color) -> Color {
    let Some(asset) = asset.filter(|asset| !asset.path.trim().is_empty()) else {
        return Color::NONE;
    };
    let opacity = asset.opacity.clamp(0.0, 1.0);
    if asset.tint_with_theme {
        let theme = theme.to_srgba();
        Color::srgba(theme.red, theme.green, theme.blue, opacity)
    } else {
        Color::srgba(1.0, 1.0, 1.0, opacity)
    }
}
