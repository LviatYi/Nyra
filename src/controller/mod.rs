mod job_manager;

pub(crate) use job_manager::ActiveJobState;

use bevy::prelude::*;
use job_manager::{JobManager, process_jobs};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct JobProcessing;

pub struct ControllerPlugin;

impl Plugin for ControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveJobState>()
            .init_resource::<JobManager>()
            .add_systems(Update, process_jobs.in_set(JobProcessing));
    }
}
