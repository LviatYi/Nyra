use bevy::prelude::*;
use serde::Deserialize;
use std::{fs, time::Duration};

const MOTION_PATH: &str = "assets/sensation/sensation_tip_overlay_motion.json";
const DEFAULT_MOTION: &str = include_str!("../../../assets/sensation/sensation_tip_overlay_motion.json");

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
    pub drain: Vec<MotionKeyframe>,
    drain_max_millis: u64,
    drain_max_next_fraction: f32,
}

impl BorderMotion {
    pub(super) fn load() -> Self {
        let source = fs::read_to_string(MOTION_PATH).unwrap_or_else(|error| {
            eprintln!("[Sensation] {MOTION_PATH}: {error}; using built-in motion");
            DEFAULT_MOTION.to_string()
        });
        Self::parse(&source).unwrap_or_else(|error| {
            eprintln!("[Sensation] {MOTION_PATH}: {error}; using built-in motion");
            Self::parse(DEFAULT_MOTION).expect("built-in border motion must be valid")
        })
    }

    fn parse(source: &str) -> Result<Self, String> {
        let motion: Self = serde_json::from_str(source).map_err(|error| error.to_string())?;
        for frames in [&motion.fill, &motion.drain] {
            if frames.len() < 2
                || frames
                    .first()
                    .is_none_or(|frame| frame.time != 0.0 || frame.value != 0.0)
                || frames
                    .last()
                    .is_none_or(|frame| frame.time != 1.0 || frame.value != 1.0)
                || frames.iter().any(|frame| {
                    !frame.time.is_finite()
                        || !frame.value.is_finite()
                        || !frame.velocity.is_finite()
                        || !(0.0..=1.0).contains(&frame.value)
                })
                || frames
                    .windows(2)
                    .any(|pair| pair[0].time >= pair[1].time || pair[0].value > pair[1].value)
            {
                return Err("motion keyframes must run from (0, 0) to (1, 1)".into());
            }
        }
        if motion.drain_max_millis == 0
            || !motion.drain_max_next_fraction.is_finite()
            || !(0.0..1.0).contains(&motion.drain_max_next_fraction)
        {
            return Err("invalid drain duration".into());
        }
        Ok(motion)
    }

    pub(super) fn drain_duration(&self, next_duration: Option<Duration>) -> Duration {
        let maximum = Duration::from_millis(self.drain_max_millis);
        next_duration
            .map(|duration| maximum.min(duration.mul_f32(self.drain_max_next_fraction)))
            .unwrap_or(maximum)
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
