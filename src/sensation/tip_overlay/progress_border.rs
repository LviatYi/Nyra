use super::{
    geometry::BorderGeometry,
    motion::{BorderMotion, sample_keyframes},
    style::{set_material_color, tip_color},
};
use crate::controller::ActiveJobState;
use crate::job::JobConfig;
use bevy::{
    animation::graph::AnimationNodeIndex,
    animation::{AnimatedBy, AnimationTargetId, animated_field},
    curve::{FunctionCurve, Interval},
    prelude::*,
};
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
    drain_duration: Duration,
}

#[derive(Resource)]
pub(super) struct BorderAnimationNodes {
    fill: AnimationNodeIndex,
    drain: AnimationNodeIndex,
}

pub(super) fn setup_border_animations(
    mut commands: Commands,
    motion: Res<BorderMotion>,
    mut clips: ResMut<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    fill_entity: Single<Entity, With<BorderFillValue>>,
    drain_entity: Single<Entity, With<BorderDrainValue>>,
) {
    let fill_target = AnimationTargetId::from_name(&Name::new("BorderFill"));
    let mut fill_clip = AnimationClip::default();
    let fill_frames = motion.fill.clone();
    fill_clip.add_curve_to_target(
        fill_target,
        AnimatableCurve::new(
            animated_field!(BorderFillValue::progress),
            FunctionCurve::new(Interval::UNIT, move |time| {
                sample_keyframes(&fill_frames, time)
            }),
        ),
    );
    let (fill_graph, fill_node) = AnimationGraph::from_clip(clips.add(fill_clip));
    commands.entity(*fill_entity).insert((
        AnimationPlayer::default(),
        AnimationGraphHandle(graphs.add(fill_graph)),
        fill_target,
        AnimatedBy(*fill_entity),
    ));

    let drain_target = AnimationTargetId::from_name(&Name::new("BorderDrain"));
    let mut drain_clip = AnimationClip::default();
    let drain_frames = motion.drain.clone();
    drain_clip.add_curve_to_target(
        drain_target,
        AnimatableCurve::new(
            animated_field!(BorderDrainValue::progress),
            FunctionCurve::new(Interval::UNIT, move |time| {
                sample_keyframes(&drain_frames, time)
            }),
        ),
    );
    let (drain_graph, drain_node) = AnimationGraph::from_clip(clips.add(drain_clip));
    commands.entity(*drain_entity).insert((
        AnimationPlayer::default(),
        AnimationGraphHandle(graphs.add(drain_graph)),
        drain_target,
        AnimatedBy(*drain_entity),
    ));
    commands.insert_resource(BorderAnimationNodes {
        fill: fill_node,
        drain: drain_node,
    });
}

pub(super) fn sync_border_playback(
    time: Res<Time<Real>>,
    state: Res<ActiveJobState>,
    jobs: Res<JobConfig>,
    motion: Res<BorderMotion>,
    nodes: Res<BorderAnimationNodes>,
    mut fill_player: Single<
        &mut AnimationPlayer,
        (With<BorderFillValue>, Without<BorderDrainValue>),
    >,
    mut drain_player: Single<
        &mut AnimationPlayer,
        (With<BorderDrainValue>, Without<BorderFillValue>),
    >,
    mut fill_value: Single<&mut BorderFillValue>,
    mut drain_value: Single<&mut BorderDrainValue>,
    outgoing_material: Single<&MeshMaterial2d<ColorMaterial>, With<BorderOutgoing>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut playback: Local<BorderPlayback>,
) {
    let Some(active_job) = state.active_job.as_ref() else {
        if playback.previous_job.take().is_some() {
            fill_player.stop_all();
            drain_player.stop_all();
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
        fill_player.stop_all();
        fill_value.progress = if countdown.is_some_and(|(_, total)| total.is_zero()) {
            1.0
        } else {
            0.0
        };
        if countdown.is_some_and(|(_, total)| !total.is_zero()) {
            fill_player.start(nodes.fill).pause();
        }

        drain_player.stop_all();
        drain_value.progress = 1.0;
        playback.draining_since = None;
        let drain_duration = motion.drain_duration(countdown.map(|(_, total)| total));
        if let Some(previous) = playback.previous_job
            && previous.ends_at.is_some_and(|end| now >= end)
            && !drain_duration.is_zero()
        {
            playback.draining_since = Some(now);
            playback.drain_duration = drain_duration;
            drain_value.progress = 0.0;
            drain_player.start(nodes.drain).pause();
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
        if !total.is_zero()
            && let Some(animation) = fill_player.animation_mut(nodes.fill)
        {
            animation.set_seek_time(
                (1.0 - remaining.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0),
            );
        }
    }

    if let Some(started_at) = playback.draining_since {
        let fraction = (now.saturating_sub(started_at).as_secs_f32()
            / playback.drain_duration.as_secs_f32())
        .clamp(0.0, 1.0);
        if fraction >= 1.0 {
            drain_player.stop_all();
            drain_value.progress = 1.0;
            playback.draining_since = None;
        } else if let Some(animation) = drain_player.animation_mut(nodes.drain) {
            animation.set_seek_time(fraction);
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
