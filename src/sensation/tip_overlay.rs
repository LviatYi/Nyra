mod geometry;
mod menu;
mod menu_keyboard;
mod motion;
mod progress_border;
mod style;
mod text_scroll;
mod view;

pub(crate) use menu::MenuActions;
pub(crate) use menu_keyboard::MenuKeyboard;

use crate::controller::{ActiveJobState, JobProcessing};
use crate::sensation::tip_overlay::menu::{ActionMenu, update_menu};
use crate::sensation::tip_overlay::progress_border::{
    BorderAnimationNodes, render_border, setup_border_animations, sync_border_playback,
    sync_countdown_text,
};
use crate::sensation::tip_overlay::text_scroll::animate_tip_text;
use crate::sensation::tip_overlay::view::{
    NoticePlacement, setup_overlay, sync_icon_placeholder, sync_notice, sync_overlay_hit_test,
    sync_overlay_position, sync_overlay_visibility,
};
use bevy::animation::{advance_animations, animate_targets};
use bevy::app::{App, PostUpdate, PreUpdate, Update};
use bevy::prelude::*;

pub(super) fn configure(app: &mut App) {
    app.init_resource::<ActionMenu>()
        .init_resource::<MenuActions>()
        .add_systems(
            PreUpdate,
            (
                setup_overlay.run_if(not(resource_exists::<NoticePlacement>)),
                setup_border_animations
                    .run_if(resource_exists::<NoticePlacement>)
                    .run_if(not(resource_exists::<BorderAnimationNodes>)),
            )
                .chain(),
        )
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
                .after(JobProcessing)
                .run_if(resource_exists::<BorderAnimationNodes>),
        )
        .add_systems(
            Update,
            (sync_overlay_position, update_menu, sync_overlay_hit_test)
                .chain()
                .after(JobProcessing)
                .run_if(resource_exists::<BorderAnimationNodes>),
        )
        .add_systems(
            PostUpdate,
            sync_border_playback
                .after(advance_animations)
                .before(animate_targets)
                .run_if(resource_exists::<BorderAnimationNodes>),
        )
        .add_systems(
            PostUpdate,
            render_border
                .after(animate_targets)
                .before(TransformSystems::Propagate)
                .run_if(resource_exists::<BorderAnimationNodes>),
        );
}
