use super::{
    geometry::{rounded_rectangle_ray_distance, triangle_mesh},
    style::{color_with_alpha, set_material_color, tip_background_color},
    view::TipBackground,
};
use crate::settings_default_values::{
    TIP_OVERLAY_CORNER_RADIUS, TIP_OVERLAY_RIPPLE_EXPAND_DURATION,
    TIP_OVERLAY_RIPPLE_FADE_DURATION, TIP_OVERLAY_RIPPLE_SEGMENTS, WINDOW_HEIGHT, WINDOW_WIDTH,
};
use bevy::{mesh::VertexAttributeValues, prelude::*};
use std::{f32::consts::PI, time::Duration};

#[derive(Component)]
pub(super) struct Ripple {
    phase: RipplePhase,
    displayed_tip_index: usize,
    origin: Vec2,
    directions: Vec<Vec2>,
    boundary_distances: Vec<f32>,
    cover_radius: f32,
}

#[derive(Clone, Copy)]
enum RipplePhase {
    Idle,
    Expanding {
        started_at: Duration,
        target_tip_index: usize,
        color: Color,
    },
    Fading {
        started_at: Duration,
        target_tip_index: usize,
        color: Color,
    },
}

impl Ripple {
    pub(super) fn new(displayed_tip_index: usize) -> Self {
        let directions = (0..=TIP_OVERLAY_RIPPLE_SEGMENTS)
            .map(|step| {
                let angle = 2.0 * PI * step as f32 / TIP_OVERLAY_RIPPLE_SEGMENTS as f32;
                Vec2::new(angle.cos(), angle.sin())
            })
            .collect::<Vec<_>>();
        let boundary_distances = vec![0.0; directions.len()];

        Self {
            phase: RipplePhase::Idle,
            displayed_tip_index,
            origin: Vec2::ZERO,
            directions,
            boundary_distances,
            cover_radius: 0.0,
        }
    }

    pub(super) fn displayed_tip_index(&self) -> usize {
        self.displayed_tip_index
    }

    pub(super) fn begin(
        &mut self,
        now: Duration,
        target_tip_index: usize,
        color: Color,
        origin: Vec2,
    ) {
        self.origin = origin;
        self.cover_radius = 0.0;
        let half_size = Vec2::new(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32) / 2.0;
        for (direction, boundary_distance) in
            self.directions.iter().zip(&mut self.boundary_distances)
        {
            *boundary_distance = rounded_rectangle_ray_distance(
                self.origin,
                *direction,
                half_size,
                TIP_OVERLAY_CORNER_RADIUS,
            );
            self.cover_radius = self.cover_radius.max(*boundary_distance);
        }
        self.phase = RipplePhase::Expanding {
            started_at: now,
            target_tip_index,
            color,
        };
    }
}

pub(super) fn ripple_mesh(ripple: &Ripple) -> Mesh {
    let positions = vec![[0.0, 0.0, 0.0]; ripple.directions.len() + 1];
    let mut indices = Vec::with_capacity((ripple.directions.len() - 1) * 3);
    for perimeter_index in 0..ripple.directions.len() - 1 {
        indices.extend_from_slice(&[0, perimeter_index as u32 + 1, perimeter_index as u32 + 2]);
    }
    triangle_mesh(positions, indices)
}

pub(super) fn animate_ripple(
    time: Res<Time<Real>>,
    ripple: Single<(&mut Ripple, &Mesh2d, &MeshMaterial2d<ColorMaterial>)>,
    background: Single<&MeshMaterial2d<ColorMaterial>, With<TipBackground>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let now = time.elapsed();
    let (mut ripple, ripple_mesh, ripple_material) = ripple.into_inner();
    let mut phase = ripple.phase;
    if let RipplePhase::Expanding {
        started_at,
        target_tip_index,
        color,
    } = phase
    {
        let linear_progress = (now.saturating_sub(started_at).as_secs_f32()
            / TIP_OVERLAY_RIPPLE_EXPAND_DURATION.as_secs_f32())
        .clamp(0.0, 1.0);
        let eased_progress = 1.0 - (1.0 - linear_progress).powi(3);
        if let Some(mut mesh) = meshes.get_mut(&ripple_mesh.0) {
            update_ripple_mesh(&mut mesh, &ripple, ripple.cover_radius * eased_progress);
        }

        if linear_progress == 1.0 {
            set_material_color(&mut materials, &background.0, tip_background_color(color));
            phase = RipplePhase::Fading {
                started_at: started_at + TIP_OVERLAY_RIPPLE_EXPAND_DURATION,
                target_tip_index,
                color,
            };
        }
    }

    if let RipplePhase::Fading {
        started_at,
        target_tip_index,
        color,
    } = phase
    {
        let fade_progress = (now.saturating_sub(started_at).as_secs_f32()
            / TIP_OVERLAY_RIPPLE_FADE_DURATION.as_secs_f32())
        .clamp(0.0, 1.0);
        let alpha = (1.0 - fade_progress).powi(2);
        set_material_color(
            &mut materials,
            &ripple_material.0,
            color_with_alpha(color, alpha),
        );
        if fade_progress == 1.0 {
            ripple.displayed_tip_index = target_tip_index;
            phase = RipplePhase::Idle;
        }
    }

    ripple.phase = phase;
}

fn update_ripple_mesh(mesh: &mut Mesh, ripple: &Ripple, radius: f32) {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    positions[0] = [ripple.origin.x, ripple.origin.y, 0.0];
    for (index, (direction, boundary_distance)) in ripple
        .directions
        .iter()
        .zip(&ripple.boundary_distances)
        .enumerate()
    {
        let point = ripple.origin + *direction * radius.min(*boundary_distance);
        positions[index + 1] = [point.x, point.y, 0.0];
    }
}
