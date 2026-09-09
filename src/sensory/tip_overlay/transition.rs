use super::{
    progress_border::{CountdownBorder, ProgressBorderGeometry},
    ripple::Ripple,
    style::{set_material_color, tip_color},
};
use crate::{job::JobConfig, sensory::job_manager::ActiveJobState};
use bevy::prelude::*;
use rand::RngExt;

/// Starts all visual state tied to a newly selected tip.
pub(super) fn begin_tip_transition(
    jobs: Res<JobConfig>,
    state: Res<ActiveJobState>,
    geometry: Res<ProgressBorderGeometry>,
    border: Single<(&mut CountdownBorder, &Mesh2d)>,
    ripple: Single<(&mut Ripple, &MeshMaterial2d<ColorMaterial>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !state.is_changed() {
        return;
    }
    let Some(active_job) = state.active_job.as_ref() else {
        return;
    };

    let (mut ripple, ripple_material) = ripple.into_inner();
    if active_job.current_index == ripple.displayed_tip_index() {
        return;
    }

    let current_color = tip_color(&jobs, active_job.current_index);
    let (mut border, border_mesh) = border.into_inner();
    let Some(mut border_mesh) = meshes.get_mut(&border_mesh.0) else {
        return;
    };

    let requested_start = rand::rng().random_range(0.0..1.0);
    let start_progress = border.restart_at(&geometry, requested_start, &mut border_mesh);
    let origin = geometry.center_at(start_progress);
    ripple.begin(
        active_job.started_at,
        active_job.current_index,
        current_color,
        origin,
    );
    set_material_color(&mut materials, &ripple_material.0, current_color);
}
