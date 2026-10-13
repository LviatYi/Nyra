use super::{
    geometry::BorderGeometry,
    style::{set_material_color, tip_color},
};
use crate::controller::ActiveJobState;
use crate::job::JobConfig;
use bevy::prelude::*;
use std::time::Duration;

#[derive(Component, Clone)]
pub(super) struct BorderProgress {
    geometry: BorderGeometry,
    progress: f32,
}

impl BorderProgress {
    pub(super) fn new(geometry: BorderGeometry) -> Self {
        Self {
            geometry,
            progress: 0.0,
        }
    }

    pub(super) fn mesh(&self) -> Mesh {
        self.geometry.progress_mesh(self.progress)
    }

    pub(super) fn outgoing_mesh(&self, start: f32) -> Mesh {
        self.geometry.progress_range_mesh(start, 1.0)
    }

    pub(super) fn head_position(&self) -> Option<Vec2> {
        self.geometry.progress_head(self.progress)
    }
}

fn eased_progress(progress: f32) -> f32 {
    const RAMP_FRACTION: f32 = 0.25;
    let cruise_speed = 1.0 / (1.0 - RAMP_FRACTION);
    let ramp_distance = RAMP_FRACTION * cruise_speed / 2.0;

    if progress < RAMP_FRACTION {
        progress * progress * cruise_speed / (2.0 * RAMP_FRACTION)
    } else if progress > 1.0 - RAMP_FRACTION {
        let remaining = 1.0 - progress;
        1.0 - remaining * remaining * cruise_speed / (2.0 * RAMP_FRACTION)
    } else {
        ramp_distance + (progress - RAMP_FRACTION) * cruise_speed
    }
}

#[derive(Component, Default, Clone)]
pub(super) struct BorderTrack;

#[derive(Component, Default, Clone)]
pub(super) struct BorderOutgoing;

#[derive(Component, Default, Clone)]
pub(super) struct BorderHead;

#[derive(Component, Default, Clone)]
pub(super) struct BorderHeadGlow(pub(super) f32);

#[derive(Component, Default, Clone)]
pub(super) struct CountdownText;

#[derive(Clone, Copy)]
struct PreviousJob {
    index: usize,
    started_at: Duration,
    ends_at: Option<Duration>,
}

#[derive(Default)]
pub(super) struct BorderTransition {
    previous_job: Option<PreviousJob>,
    draining_since: Option<Duration>,
    drain_duration: Duration,
}

pub(super) fn 
update_countdown(
    time: Res<Time<Real>>,
    state: Res<ActiveJobState>,
    jobs: Res<JobConfig>,
    border: Single<(Entity, &mut BorderProgress, &Mesh2d)>,
    outgoing: Single<(Entity, &Mesh2d, &MeshMaterial2d<ColorMaterial>), With<BorderOutgoing>>,
    head: Single<(Entity, &mut Transform), With<BorderHead>>,
    mut visibilities: Query<&mut Visibility>,
    mut label: Single<&mut Text, With<CountdownText>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut transition: Local<BorderTransition>,
) {
    let (border_entity, mut border, mesh_handle) = border.into_inner();
    let (outgoing_entity, outgoing_mesh, outgoing_material) = outgoing.into_inner();
    let Some(active_job) = state.active_job.as_ref() else {
        transition.previous_job = None;
        transition.draining_since = None;
        *visibilities.get_mut(border_entity).unwrap() = Visibility::Hidden;
        *visibilities.get_mut(outgoing_entity).unwrap() = Visibility::Hidden;
        return;
    };
    let now = time.elapsed();
    let countdown = active_job.countdown_at(now);
    let ends_at = countdown.map(|(_, total)| active_job.started_at.saturating_add(total));
    if let Some(previous) = transition.previous_job
        && (previous.index != active_job.current_index
            || previous.started_at != active_job.started_at)
    {
        transition.draining_since = None;
        *visibilities.get_mut(outgoing_entity).unwrap() = Visibility::Hidden;
        let max_drain_duration = Duration::from_millis(180);
        let drain_duration = countdown
            .map(|(_, total)| max_drain_duration.min(total / 4))
            .unwrap_or(max_drain_duration);
        if previous.ends_at.is_some_and(|end| now >= end) && !drain_duration.is_zero() {
            transition.draining_since = Some(now);
            transition.drain_duration = drain_duration;
            set_material_color(
                &mut materials,
                &outgoing_material.0,
                tip_color(&jobs, previous.index),
            );
        }
    }
    transition.previous_job = Some(PreviousJob {
        index: active_job.current_index,
        started_at: active_job.started_at,
        ends_at,
    });
    if let Some(started_at) = transition.draining_since {
        let fraction = (now.saturating_sub(started_at).as_secs_f32()
            / transition.drain_duration.as_secs_f32())
        .clamp(0.0, 1.0);
        if fraction >= 1.0 {
            transition.draining_since = None;
            *visibilities.get_mut(outgoing_entity).unwrap() = Visibility::Hidden;
        } else {
            if let Some(mut mesh) = meshes.get_mut(&outgoing_mesh.0) {
                *mesh = border.outgoing_mesh(fraction);
            }
            *visibilities.get_mut(outgoing_entity).unwrap() = Visibility::Visible;
        }
    }
    let (progress, caption) = match countdown {
        Some((remaining, total)) => {
            let progress = if total.is_zero() {
                1.0
            } else {
                eased_progress(
                    (1.0 - remaining.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0),
                )
            };
            (progress, format!("{:06.3}s", remaining.as_secs_f64().min(99.999)))
        }
        None => (0.0, "--".to_string()),
    };
    if label.0 != caption {
        label.0 = caption;
    }
    if border.progress != progress {
        border.progress = progress;
        if progress > 0.0 {
            if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
                *mesh = border.mesh();
            }
        }
    }
    *visibilities.get_mut(border_entity).unwrap() = if progress > 0.0 {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    let (head_entity, mut head_transform) = head.into_inner();
    if let Some(position) = border.head_position() {
        head_transform.translation.x = position.x;
        head_transform.translation.y = position.y;
        *visibilities.get_mut(head_entity).unwrap() = Visibility::Visible;
    } else {
        *visibilities.get_mut(head_entity).unwrap() = Visibility::Hidden;
    }
}
