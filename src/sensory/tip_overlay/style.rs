use crate::{job::JobConfig, settings_default_values::*};
use bevy::{prelude::*, sprite_render::AlphaMode2d};

pub(super) fn tip_color(jobs: &JobConfig, tip_index: usize) -> Color {
    Color::Srgba(
        Srgba::hex(jobs.resolved_color_at(tip_index))
            .expect("tip colors are validated before startup"),
    )
}

pub(super) fn tip_background_color(color: Color) -> Color {
    let color = color.to_srgba();
    Color::srgba(
        color.red * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        color.green * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        color.blue * TIP_OVERLAY_BACKGROUND_BRIGHTNESS,
        TIP_OVERLAY_BACKGROUND_ALPHA,
    )
}

pub(super) fn color_with_alpha(color: Color, alpha: f32) -> Color {
    let color = color.to_srgba();
    Color::srgba(color.red, color.green, color.blue, alpha)
}

pub(super) fn overlay_material(color: Color) -> ColorMaterial {
    ColorMaterial {
        color,
        // Keep every overlay mesh in the transparent phase so Z controls their order.
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
