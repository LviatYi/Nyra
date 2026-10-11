use crate::job::JobConfig;
use bevy::{prelude::*, sprite_render::AlphaMode2d};

pub(super) fn tip_color(jobs: &JobConfig, tip_index: usize) -> Color {
    Color::Srgba(
        Srgba::hex(jobs.resolved_color_at(tip_index))
            .expect("tip colors are validated before startup"),
    )
}

pub(super) fn border_track_color(theme: Color) -> Color {
    let theme = theme.to_srgba();
    Color::srgba(theme.red * 0.25, theme.green * 0.25, theme.blue * 0.25, 1.0)
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
