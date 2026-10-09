use super::geometry::BorderGeometry;
use crate::controller::ActiveJobState;
use bevy::prelude::*;

#[derive(Component)]
pub(super) struct BorderProgress {
    geometry: BorderGeometry,
    progress: f32,
}

impl BorderProgress {
    pub(super) fn new(geometry: BorderGeometry) -> Self {
        Self {
            geometry,
            progress: 1.0,
        }
    }

    pub(super) fn mesh(&self) -> Mesh {
        self.geometry.progress_mesh(self.progress)
    }
}

#[derive(Component, Default, Clone)]
pub(super) struct CountdownText;

pub(super) fn update_countdown(
    time: Res<Time<Real>>,
    state: Res<ActiveJobState>,
    border: Single<(&mut BorderProgress, &Mesh2d)>,
    mut label: Single<&mut Text, With<CountdownText>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(active_job) = state.active_job.as_ref() else {
        return;
    };
    let (mut border, mesh_handle) = border.into_inner();
    let (progress, caption) = match active_job.countdown_at(time.elapsed()) {
        Some((remaining, total)) => {
            let progress = if total.is_zero() {
                0.0
            } else {
                (remaining.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
            };
            (progress, format!("{:.3}s", remaining.as_secs_f64()))
        }
        None => (1.0, "--".to_string()),
    };
    if label.0 != caption {
        label.0 = caption;
    }
    if border.progress != progress {
        border.progress = progress;
        if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
            *mesh = border.mesh();
        }
    }
}
