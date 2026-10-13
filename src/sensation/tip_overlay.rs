mod geometry;
mod progress_border;
mod style;
mod text_scroll;
mod view;
mod visual_config;

use crate::controller::{ActiveJobState, JobProcessing};
use crate::sensation::tip_overlay::progress_border::update_countdown;
use crate::sensation::tip_overlay::text_scroll::animate_tip_text;
use crate::sensation::tip_overlay::view::{
    setup_overlay, sync_icon_placeholder, sync_notice, sync_overlay_hit_test,
    sync_overlay_position, sync_overlay_visibility,
};
use crate::sensation::tip_overlay::visual_config::RotationNoticeVisualConfig;
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
