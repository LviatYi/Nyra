use bevy::prelude::*;
use serde::Deserialize;
use std::{fs, time::Duration};

const MOTION_PATH: &str = "assets/sensation/sensation_tip_overlay_motion.json";

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MotionKeyframe {
    pub time: f32,
    pub value: f32,
    pub velocity: f32,
}

#[derive(Resource, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BorderMotion {
    pub fill: Vec<MotionKeyframe>,
    drain_millis: u64,
}

impl BorderMotion {
    pub(super) fn load() -> Self {
        let source = fs::read_to_string(MOTION_PATH)
            .unwrap_or_else(|error| panic!("[Sensation] {MOTION_PATH}: {error}"));
        Self::parse(&source).unwrap_or_else(|error| panic!("[Sensation] {MOTION_PATH}: {error}"))
    }

    fn parse(source: &str) -> Result<Self, String> {
        let motion: Self = serde_json::from_str(source).map_err(|error| error.to_string())?;

        if motion.fill.len() < 2
            || motion
                .fill
                .first()
                .is_none_or(|frame| frame.time != 0.0 || frame.value != 0.0)
            || motion
                .fill
                .last()
                .is_none_or(|frame| frame.time != 1.0 || frame.value != 1.0)
            || motion.fill.iter().any(|frame| {
                !frame.time.is_finite()
                    || !frame.value.is_finite()
                    || !frame.velocity.is_finite()
                    || !(0.0..=1.0).contains(&frame.value)
            })
            || motion
                .fill
                .windows(2)
                .any(|pair| pair[0].time >= pair[1].time || pair[0].value > pair[1].value)
        {
            return Err("motion keyframes must run from (0, 0) to (1, 1)".into());
        }

        if motion.drain_millis == 0 {
            return Err("invalid drain duration".into());
        }

        Ok(motion)
    }

    pub(super) fn drain_duration(&self) -> Duration {
        Duration::from_millis(self.drain_millis)
    }
}

pub(super) fn sample_keyframes(frames: &[MotionKeyframe], time: f32) -> f32 {
    let next = frames.partition_point(|frame| frame.time <= time);
    let index = next.saturating_sub(1).min(frames.len() - 2);
    let start = &frames[index];
    let end = &frames[index + 1];
    let span = end.time - start.time;
    let t = ((time - start.time) / span).clamp(0.0, 1.0);
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * start.value
        + (t3 - 2.0 * t2 + t) * span * start.velocity
        + (-2.0 * t3 + 3.0 * t2) * end.value
        + (t3 - t2) * span * end.velocity
}
