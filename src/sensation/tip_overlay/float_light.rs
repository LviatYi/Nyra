use super::{geometry::triangle_mesh, motion::DRAIN_DURATION};
use crate::{
    controller::ActiveJobState,
    sensation::sensation_config::{SensationConfig, SensationConfigHandle, layer_color},
    settings_default_values::{WINDOW_HEIGHT, WINDOW_WIDTH},
};
use bevy::prelude::*;
use std::time::Duration;

const FADE_DURATION: Duration = Duration::from_millis(90);

#[derive(Component, Default, Clone)]
pub(super) struct FloatLight {
    pub(super) started_at: Option<Duration>,
    pub(super) theme: Color,
}

pub(super) fn float_light_mesh(reveal: f32) -> Mesh {
    let half_width = WINDOW_WIDTH as f32 / 2.0;
    let half_height = WINDOW_HEIGHT as f32 / 2.0;
    let right = -half_width + WINDOW_WIDTH as f32 * reveal;
    let mut mesh = triangle_mesh(
        vec![
            [-half_width, -half_height, 0.0],
            [right, -half_height, 0.0],
            [right, half_height, 0.0],
            [-half_width, half_height, 0.0],
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 1.0], [reveal, 1.0], [reveal, 0.0], [0.0, 0.0]],
    );
    mesh
}

pub(super) fn animate_float_light(
    time: Res<Time<Real>>,
    state: Res<ActiveJobState>,
    config: Res<SensationConfigHandle>,
    visual_assets: Res<Assets<SensationConfig>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    light: Single<(
        &mut FloatLight,
        &Mesh2d,
        &MeshMaterial2d<ColorMaterial>,
        &mut Visibility,
    )>,
) {
    let (mut light, mesh_handle, material_handle, mut visibility) = light.into_inner();
    let elapsed = light
        .started_at
        .map(|start| time.elapsed().saturating_sub(start));
    let animation = elapsed.filter(|elapsed| *elapsed < DRAIN_DURATION + FADE_DURATION);
    let Some(elapsed) = animation.filter(|_| state.active_job.is_some()) else {
        if light.started_at.take().is_some() {
            if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
                *mesh = float_light_mesh(0.0);
            }
            if let Some(mut material) = materials.get_mut(&material_handle.0) {
                material.color = Color::NONE;
            }
        }
        *visibility = Visibility::Hidden;
        return;
    };
    let Some(visuals) = config.resolve(&visual_assets, &asset_server) else {
        return;
    };
    let reveal = (elapsed.as_secs_f32() / DRAIN_DURATION.as_secs_f32()).min(1.0);
    let alpha = 1.0
        - (elapsed.saturating_sub(DRAIN_DURATION).as_secs_f32() / FADE_DURATION.as_secs_f32())
            .min(1.0);
    let color = layer_color(visuals.float_light.as_ref(), light.theme);
    if let Some(mut material) = materials.get_mut(&material_handle.0) {
        material.color = color.with_alpha(color.alpha() * alpha);
    }
    if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
        *mesh = float_light_mesh(reveal);
    }
    *visibility = if reveal > 0.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}
