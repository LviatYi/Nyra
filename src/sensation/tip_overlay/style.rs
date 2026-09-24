use crate::{job::JobConfig, settings_default_values::*};
use bevy::{
    color::LinearRgba,
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d},
};

const OVERLAY_SHADER_PATH: &str = "shaders/tip_overlay_material.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct OverlayMaterial {
    #[uniform(0)]
    color: LinearRgba,
    #[uniform(1)]
    clip_half_size: Vec2,
    #[uniform(2)]
    corner_radius: f32,
}

impl Material2d for OverlayMaterial {
    fn fragment_shader() -> ShaderRef {
        OVERLAY_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

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

pub(super) fn overlay_material(color: Color) -> OverlayMaterial {
    OverlayMaterial {
        color: color.into(),
        clip_half_size: Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) / 2.0,
        corner_radius: TIP_OVERLAY_CORNER_RADIUS,
    }
}

pub(super) fn set_material_color(
    materials: &mut Assets<OverlayMaterial>,
    handle: &Handle<OverlayMaterial>,
    color: Color,
) {
    if let Some(mut material) = materials.get_mut(handle) {
        material.color = color.into();
    }
}
