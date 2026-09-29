use crate::{job::JobConfig, settings_default_values::*};
use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dKey},
};

const ROUNDED_RECTANGLE_MASK_SHADER_PATH: &str = "shaders/rounded_rectangle_mask.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct RoundedRectangleMaskMaterial {
    #[uniform(0)]
    half_size: Vec2,
    #[uniform(1)]
    corner_radius: f32,
}

impl Material2d for RoundedRectangleMaskMaterial {
    fn fragment_shader() -> ShaderRef {
        ROUNDED_RECTANGLE_MASK_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let Some(target) = descriptor
            .fragment
            .as_mut()
            .and_then(|fragment| fragment.targets.first_mut())
            .and_then(Option::as_mut)
        else {
            return Ok(());
        };
        let destination_mask = BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::SrcAlpha,
            operation: BlendOperation::Add,
        };
        // The shader writes coverage to source alpha, so this multiplies the
        // accumulated destination RGB and alpha by that coverage.
        target.blend = Some(BlendState {
            color: destination_mask,
            alpha: destination_mask,
        });
        Ok(())
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

pub(super) fn overlay_color_material(color: Color) -> ColorMaterial {
    ColorMaterial {
        color,
        alpha_mode: AlphaMode2d::Blend,
        ..default()
    }
}

pub(super) fn rounded_rectangle_mask_material() -> RoundedRectangleMaskMaterial {
    RoundedRectangleMaskMaterial {
        half_size: Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) / 2.0,
        corner_radius: TIP_OVERLAY_CORNER_RADIUS,
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
