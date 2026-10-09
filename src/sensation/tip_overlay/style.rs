use crate::{job::JobConfig, settings_default_values::*};
use bevy::{prelude::*, sprite_render::AlphaMode2d};

pub(super) fn tip_color(jobs: &JobConfig, tip_index: usize) -> Color {
    Color::Srgba(
        Srgba::hex(jobs.resolved_color_at(tip_index))
            .expect("tip colors are validated before startup"),
    )
}

pub(super) fn base_color(theme: Color, opacity: f32) -> Color {
    let theme = theme.to_srgba();
    Color::srgba(
        theme.red * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        theme.green * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        theme.blue * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        opacity.clamp(0.0, 1.0),
    )
}

pub(super) fn color_material(color: Color) -> ColorMaterial {
    ColorMaterial {
        color,
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    }
}

pub(super) fn set_material_color(
    materials: &mut Assets<ColorMaterial>,
    handle: &Handle<ColorMaterial>,
    color: Color,
) {
    if let Some(mut material) = materials.get_mut(handle) {
        material.color = color;
    }
}
