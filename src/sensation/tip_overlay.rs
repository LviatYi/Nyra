mod geometry;
mod menu;
mod motion;
mod progress_border;
mod style;
mod text_scroll;
mod view;

pub(crate) use menu::MenuActions;

use crate::controller::{ActiveJobState, JobProcessing};
use crate::sensation::tip_overlay::menu::ActionMenu;
use crate::sensation::tip_overlay::progress_border::{
    render_border, sync_border_playback, sync_countdown_text,
};
use crate::sensation::tip_overlay::text_scroll::animate_tip_text;
use crate::sensation::tip_overlay::view::{
    NoticePlacement, setup_overlay, sync_icon_placeholder, sync_notice, sync_overlay_hit_test,
    sync_overlay_position, sync_overlay_visibility,
};
use bevy::app::{App, PreUpdate, Update};
use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MenuProcessing;

pub(super) fn configure(app: &mut App) {
    app.init_resource::<ActionMenu>()
        .init_resource::<MenuActions>()
        .add_systems(
            PreUpdate,
            setup_overlay.run_if(not(resource_exists::<NoticePlacement>)),
        )
        .add_systems(
            Update,
            (
                (
                    (sync_notice, sync_overlay_visibility)
                        .chain()
                        .run_if(resource_changed::<ActiveJobState>),
                    sync_countdown_text,
                    sync_icon_placeholder,
                    animate_tip_text,
                )
                    .chain()
                    .after(JobProcessing)
                    .run_if(resource_exists::<NoticePlacement>),
                (
                    sync_overlay_position,
                    menu::update_menu.in_set(MenuProcessing),
                    sync_overlay_hit_test,
                )
                    .chain()
                    .after(JobProcessing)
                    .run_if(resource_exists::<NoticePlacement>),
                (sync_border_playback, render_border)
                    .chain()
                    .after(JobProcessing)
                    .run_if(resource_exists::<NoticePlacement>),
            ),
        );
}
