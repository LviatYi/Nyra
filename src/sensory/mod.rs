mod job_manager;
pub mod tip_overlay;

use crate::sensory::job_manager::{ActiveJobState, JobManager};
use bevy::prelude::*;

pub struct SensoryPlugin;

impl Plugin for SensoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveJobState>()
            .init_resource::<JobManager>();
        tip_overlay::configure(app);
    }
}
