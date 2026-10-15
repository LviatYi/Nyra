use bevy::{
    animation::animation_curves::AnimationCompatibleCurve,
    curve::{FunctionCurve, Interval},
    prelude::*,
};
use std::time::Duration;

pub(super) const DRAIN_DURATION: Duration = Duration::from_millis(180);

pub(super) fn fill_curve() -> impl AnimationCompatibleCurve<f32> {
    let accelerate_until = 0.25;
    let decelerate_from = 0.75;
    let accelerate = EasingCurve::new(0.0, 1.0 / 6.0, EaseFunction::QuadraticIn);
    let cruise = EasingCurve::new(1.0 / 6.0, 5.0 / 6.0, EaseFunction::Linear);
    let decelerate = EasingCurve::new(5.0 / 6.0, 1.0, EaseFunction::QuadraticOut);

    FunctionCurve::new(Interval::UNIT, move |time| {
        if time < accelerate_until {
            accelerate.sample_clamped(time / accelerate_until)
        } else if time < decelerate_from {
            cruise.sample_clamped((time - accelerate_until) / (decelerate_from - accelerate_until))
        } else {
            decelerate.sample_clamped((time - decelerate_from) / (1.0 - decelerate_from))
        }
    })
}
