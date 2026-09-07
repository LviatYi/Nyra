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
