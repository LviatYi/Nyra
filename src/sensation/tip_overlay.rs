mod geometry;
mod motion;
mod progress_border;
mod style;
mod text_scroll;
mod view;
mod visual_config;

use crate::controller::{ActiveJobState, JobProcessing};
use crate::sensation::tip_overlay::motion::BorderMotion;
use crate::sensation::tip_overlay::progress_border::{
    render_border, setup_border_animations, sync_border_playback, sync_countdown_text,
};
use crate::sensation::tip_overlay::text_scroll::animate_tip_text;
use crate::sensation::tip_overlay::view::{
    setup_overlay, sync_icon_placeholder, sync_notice, sync_overlay_hit_test,
    sync_overlay_position, sync_overlay_visibility,
};
use crate::sensation::tip_overlay::visual_config::RotationNoticeVisualConfig;
use bevy::prelude::*;
use bevy::{
    animation::{advance_animations, animate_targets},
    transform::TransformSystems,
};

pub(super) fn configure(app: &mut App) {
    #[cfg(target_os = "windows")]
    app.add_observer(view::mark_overlay_window_non_rude)
        .add_observer(view::unmark_overlay_window_non_rude);

    app.insert_resource(RotationNoticeVisualConfig::load())
        .insert_resource(BorderMotion::load())
        .add_systems(Startup, setup_overlay)
        .add_systems(PostStartup, setup_border_animations)
        .add_systems(
            Update,
            (
                (sync_notice, sync_overlay_visibility)
                    .chain()
                    .run_if(resource_changed::<ActiveJobState>),
                sync_countdown_text,
                sync_icon_placeholder,
                animate_tip_text,
            )
                .chain()
                .after(JobProcessing),
        )
        .add_systems(
            Update,
            (sync_overlay_position, sync_overlay_hit_test)
                .chain()
                .after(JobProcessing),
        )
        .add_systems(
            PostUpdate,
            sync_border_playback
                .after(advance_animations)
                .before(animate_targets),
        )
        .add_systems(
            PostUpdate,
            render_border
                .after(animate_targets)
                .before(TransformSystems::Propagate),
        );
}
