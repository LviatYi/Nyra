mod job_manager;

pub(crate) use job_manager::{ActiveJobState, JobManager};

use bevy::prelude::*;
use job_manager::{jobs_are_awake, poll_initiatives, process_jobs, update_job_sleep};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct JobProcessing;

pub struct ControllerPlugin;

impl Plugin for ControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveJobState>()
            .init_resource::<JobManager>()
            .add_systems(
                Update,
                (
                    update_job_sleep,
                    poll_initiatives.run_if(jobs_are_awake),
                    process_jobs.run_if(jobs_are_awake),
                )
                    .chain()
                    .in_set(JobProcessing),
            );
    }
}
