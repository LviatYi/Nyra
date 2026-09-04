pub mod overlay;
mod job;

use crate::sensory::overlay::{drag_overlay, process_jobs, render_job, setup_overlay};
use bevy::prelude::*;
use crate::sensory::job::JobSensoryState;

pub struct SensoryPlugin;

impl Plugin for SensoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JobSensoryState>()
            .add_systems(Startup, setup_overlay)
           .add_systems(Update, (process_jobs, render_job).chain())
           .add_systems(Update, drag_overlay);
    }
}
