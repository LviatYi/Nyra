use crate::job::JobConfig;
use bevy::prelude::*;
use std::time::Duration;

pub struct FocusState {
    pub current_index: usize,
    pub started_at: Duration,
}

impl FocusState {
    pub fn elapsed_at(&self, now: Duration) -> Duration {
        now.saturating_sub(self.started_at)
    }
}

#[derive(Resource, Default)]
pub struct JobSensoryState {
    pub focus_state: Option<FocusState>,
}

impl JobSensoryState {
    pub fn restart_at(&mut self, index: usize, now: Duration) {
        self.focus_state = Some(FocusState {
            current_index: index,
            started_at: now,
        });
    }

    pub fn restart(&mut self, now: Duration) {
        self.restart_at(0, now);
    }
}

pub(super) fn process_jobs(
    time: Res<Time<Real>>,
    jobs: Res<JobConfig>,
    mut state: ResMut<JobSensoryState>,
) {
    let changed = process_jobs_at(&jobs, state.bypass_change_detection(), time.elapsed());
    if changed {
        state.set_changed();
    }
}

fn process_jobs_at(jobs: &JobConfig, state: &mut JobSensoryState, now: Duration) -> bool {
    match state.focus_state.as_ref() {
        None => {
            if jobs.is_empty() {
                false
            } else {
                state.restart(now);
                true
            }
        }
        Some(focus_state) => match jobs.0.tips.get(focus_state.current_index) {
            None => {
                state.restart(now);
                true
            }
            Some(tip) => {
                if focus_state.elapsed_at(now) >= Duration::from_secs(tip.show_time()) {
                    let next_index = (focus_state.current_index + 1) % jobs.0.tips.len();
                    state.restart_at(next_index, now);
                    true
                } else {
                    false
                }
            }
        },
    }
}
