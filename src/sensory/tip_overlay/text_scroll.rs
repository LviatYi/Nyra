use super::view::TipText;
use crate::settings_default_values::{
    TIP_TEXT_SCROLL_ACCELERATION, TIP_TEXT_SCROLL_END_WAIT_DURATION,
    TIP_TEXT_SCROLL_RETURN_DECELERATION, TIP_TEXT_SCROLL_RETURN_SPEED, TIP_TEXT_SCROLL_SPEED,
    TIP_TEXT_SCROLL_START_WAIT_DURATION,
};
use bevy::prelude::*;

#[derive(Component)]
pub(super) struct TipTextViewport;

#[derive(Component, Default)]
pub(super) struct TextScroll {
    elapsed: f32,
    distance: f32,
}

/// A leg with optional acceleration and fixed braking, cruising when distance permits.
struct ScrollLeg {
    distance: f32,
    speed: f32,
    acceleration_time: f32,
    deceleration_time: f32,
}

impl ScrollLeg {
    fn new(distance: f32, speed: f32, acceleration: f32) -> Self {
        // Reserve half the distance for braking when the target speed is unreachable.
        let speed = speed.min((distance * acceleration).sqrt());
        Self {
            distance,
            speed,
            acceleration_time: speed / acceleration,
            deceleration_time: speed / acceleration,
        }
    }

    fn returning(distance: f32, speed: f32, deceleration: f32) -> Self {
        // Start at the highest speed that can stop within the available distance.
        let speed = speed.min((2.0 * distance * deceleration).sqrt());
        Self {
            distance,
            speed,
            acceleration_time: 0.0,
            deceleration_time: speed / deceleration,
        }
    }

    fn duration(&self) -> f32 {
        self.distance / self.speed + 0.5 * (self.acceleration_time + self.deceleration_time)
    }

    fn offset(&self, elapsed: f32) -> f32 {
        let elapsed = elapsed.clamp(0.0, self.duration());
        if elapsed < self.acceleration_time {
            0.5 * self.speed / self.acceleration_time * elapsed * elapsed
        } else if elapsed < self.duration() - self.deceleration_time {
            self.speed * (elapsed - 0.5 * self.acceleration_time)
        } else {
            let remaining = self.duration() - elapsed;
            self.distance - 0.5 * self.speed / self.deceleration_time * remaining * remaining
        }
    }
}

pub(super) fn animate_tip_text(
    time: Res<Time<Real>>,
    viewport: Single<&ComputedNode, With<TipTextViewport>>,
    text: Single<(Ref<Text>, &ComputedNode, &mut UiTransform, &mut TextScroll), With<TipText>>,
) {
    let (text, node, mut transform, mut scroll) = text.into_inner();
    if text.is_changed() {
        *scroll = TextScroll::default();
        transform.translation = Val2::ZERO;
        // UI layout measures the new content in PostUpdate. Use it next frame.
        return;
    }

    let text_width = node.size().x * node.inverse_scale_factor();
    let viewport_width = viewport.size().x * viewport.inverse_scale_factor();
    let distance = (text_width - viewport_width).max(0.0);
    if distance != scroll.distance {
        scroll.distance = distance;
        scroll.elapsed = 0.0;
    }
    if distance == 0.0 {
        transform.translation = Val2::ZERO;
        return;
    }

    let forward = ScrollLeg::new(
        distance,
        TIP_TEXT_SCROLL_SPEED,
        TIP_TEXT_SCROLL_ACCELERATION,
    );
    let backward = ScrollLeg::returning(
        distance,
        TIP_TEXT_SCROLL_RETURN_SPEED,
        TIP_TEXT_SCROLL_RETURN_DECELERATION,
    );
    let start_wait = TIP_TEXT_SCROLL_START_WAIT_DURATION.as_secs_f32();
    let forward_end = start_wait + forward.duration();
    let return_start = forward_end + TIP_TEXT_SCROLL_END_WAIT_DURATION.as_secs_f32();
    let cycle = return_start + backward.duration();
    scroll.elapsed = (scroll.elapsed + time.delta_secs()).rem_euclid(cycle);

    let offset = if scroll.elapsed < start_wait {
        0.0
    } else if scroll.elapsed < forward_end {
        // Read towards the right: accelerate, cruise, then decelerate.
        forward.offset(scroll.elapsed - start_wait)
    } else if scroll.elapsed < return_start {
        distance
    } else {
        // Return immediately at peak speed, then brake to a stop at the beginning.
        distance - backward.offset(scroll.elapsed - return_start)
    };
    transform.translation = Val2::px(-offset.clamp(0.0, distance), 0.0);
}
