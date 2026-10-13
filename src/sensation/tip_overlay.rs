mod geometry;
mod progress_border;
mod style;
mod text_scroll;
mod view;
mod visual_config;

use self::{
    progress_border::update_countdown,
    text_scroll::animate_tip_text,
    view::{
        setup_overlay, sync_icon_placeholder, sync_notice, sync_overlay_hit_test,
        sync_overlay_position, sync_overlay_visibility,
    },
    visual_config::RotationNoticeVisualConfig,
};
use crate::controller::{ActiveJobState, JobProcessing};
use bevy::prelude::*;

pub(super) fn configure(app: &mut App) {
    #[cfg(target_os = "windows")]
    app.add_observer(view::mark_overlay_window_non_rude)
        .add_observer(view::unmark_overlay_window_non_rude);

    app.insert_resource(RotationNoticeVisualConfig::load())
        .add_systems(Startup, setup_overlay)
        .add_systems(
            Update,
            (
                (sync_notice, sync_overlay_visibility)
                    .chain()
                    .run_if(resource_changed::<ActiveJobState>),
                update_countdown,
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
        );
}
