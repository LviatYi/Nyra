use super::{
    float_light::FloatLight,
    geometry::BorderGeometry,
    motion::{DRAIN_DURATION, fill_progress},
    style::{set_material_color, tip_color},
};
use crate::controller::ActiveJobState;
use crate::job::JobConfig;
use bevy::prelude::*;
use std::time::Duration;

#[derive(Component, Clone)]
pub(super) struct BorderProgress {
    geometry: BorderGeometry,
}

impl BorderProgress {
    pub(super) fn new(geometry: BorderGeometry) -> Self {
        Self { geometry }
    }

    pub(super) fn mesh(&self, progress: f32) -> Mesh {
        self.geometry.progress_mesh(progress)
    }

    pub(super) fn outgoing_mesh(&self, start: f32) -> Mesh {
        self.geometry.progress_range_mesh(start, 1.0)
    }

    pub(super) fn head_position(&self, progress: f32) -> Vec2 {
        self.geometry.progress_head(progress)
    }
}

#[derive(Component, Default, Clone, Reflect)]
pub(super) struct BorderFillValue {
    progress: f32,
}

#[derive(Component, Clone, Reflect)]
pub(super) struct BorderDrainValue {
    progress: f32,
}

impl Default for BorderDrainValue {
    fn default() -> Self {
        Self { progress: 1.0 }
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
pub(super) struct BorderPlayback {
    previous_job: Option<PreviousJob>,
    draining_since: Option<Duration>,
}

pub(super) fn sync_border_playback(
    time: Res<Time<Real>>,
    state: Res<ActiveJobState>,
    jobs: Res<JobConfig>,
    mut fill_value: Single<&mut BorderFillValue>,
    mut drain_value: Single<&mut BorderDrainValue>,
    mut float_light: Single<&mut FloatLight>,
    outgoing_material: Single<&MeshMaterial2d<ColorMaterial>, With<BorderOutgoing>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut playback: Local<BorderPlayback>,
) {
    let Some(active_job) = state.active_job.as_ref() else {
        if playback.previous_job.take().is_some() {
            fill_value.progress = 0.0;
            drain_value.progress = 1.0;
            playback.draining_since = None;
        }
        return;
    };
    let now = time.elapsed();
    let countdown = active_job.countdown_at(now);
    let ends_at = countdown.map(|(_, total)| active_job.started_at.saturating_add(total));
    let changed = playback.previous_job.is_none_or(|previous| {
        previous.index != active_job.current_index || previous.started_at != active_job.started_at
    });
    if changed {
        float_light.started_at = playback.previous_job.map(|_| now);
        float_light.theme = tip_color(&jobs, active_job.current_index);
        fill_value.progress = if countdown.is_some_and(|(_, total)| total.is_zero()) {
            1.0
        } else {
            0.0
        };
        drain_value.progress = 1.0;
        playback.draining_since = None;
        if let Some(previous) = playback.previous_job
            && previous.ends_at.is_some_and(|end| now >= end)
            && countdown.is_none_or(|(_, total)| DRAIN_DURATION <= total)
        {
            playback.draining_since = Some(now);
            drain_value.progress = 0.0;
            set_material_color(
                &mut materials,
                &outgoing_material.0,
                tip_color(&jobs, previous.index),
            );
        }
    }
    playback.previous_job = Some(PreviousJob {
        index: active_job.current_index,
        started_at: active_job.started_at,
        ends_at,
    });

    if let Some((remaining, total)) = countdown {
        if !total.is_zero() {
            let fraction = (1.0 - remaining.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0);
            fill_value.progress = fill_progress(fraction);
        }
    }

    if let Some(started_at) = playback.draining_since {
        let fraction = (now.saturating_sub(started_at).as_secs_f32()
            / DRAIN_DURATION.as_secs_f32())
        .clamp(0.0, 1.0);
        if fraction >= 1.0 {
            drain_value.progress = 1.0;
            playback.draining_since = None;
        } else {
            drain_value.progress = fraction;
        }
    }
}

pub(super) fn sync_countdown_text(
    time: Res<Time<Real>>,
    state: Res<ActiveJobState>,
    mut label: Single<&mut Text, With<CountdownText>>,
) {
    let caption = state
        .active_job
        .as_ref()
        .and_then(|job| job.countdown_at(time.elapsed()))
        .map(|(remaining, _)| format!("{:06.3}s", remaining.as_secs_f64().min(99.999)))
        .unwrap_or_else(|| "--".into());
    if label.0 != caption {
        label.0 = caption;
    }
}

pub(super) fn render_border(
    border: Single<(&BorderProgress, Ref<BorderFillValue>, &Mesh2d)>,
    outgoing: Single<(Ref<BorderDrainValue>, &Mesh2d), With<BorderOutgoing>>,
    mut head: Single<&mut Transform, With<BorderHead>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (border, fill, fill_mesh) = border.into_inner();
    let (drain, drain_mesh) = outgoing.into_inner();
    if fill.is_changed()
        && let Some(mut mesh) = meshes.get_mut(&fill_mesh.0)
    {
        *mesh = border.mesh(fill.progress);
    }
    if drain.is_changed()
        && let Some(mut mesh) = meshes.get_mut(&drain_mesh.0)
    {
        *mesh = border.outgoing_mesh(drain.progress);
    }
    let position = border.head_position(fill.progress);
    head.translation.x = position.x;
    head.translation.y = position.y;
}
