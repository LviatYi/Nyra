mod geometry;
mod progress_border;
mod ripple;
mod style;
mod transition;
mod view;

use self::{
    progress_border::update_countdown_border,
    ripple::animate_ripple,
    transition::begin_tip_transition,
    view::{drag_overlay, setup_overlay, sync_tip_colors, sync_tip_text},
};
use super::job_manager::process_jobs;
use bevy::prelude::*;

pub(super) fn configure(app: &mut App) {
    app.add_systems(Startup, setup_overlay)
        .add_systems(
            Update,
            (
                process_jobs,
                sync_tip_text,
                sync_tip_colors,
                begin_tip_transition,
                animate_ripple,
                update_countdown_border,
            )
                .chain(),
        )
        .add_systems(Update, drag_overlay);
}
