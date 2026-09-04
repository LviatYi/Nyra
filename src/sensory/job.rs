use bevy::prelude::*;
use std::time::Instant;

pub struct FocusState {
    pub current_index: usize,
    pub focus_at_time: Instant,
}

#[derive(Resource, Default)]
pub struct JobSensoryState {
    pub focus_state: Option<FocusState>,
}

impl JobSensoryState {
    pub fn restart_at(&mut self, index: usize) {
        self.focus_state = Some(FocusState {
            current_index: index,
            focus_at_time: Instant::now(),
        });
    }

    pub fn restart(&mut self) {
        self.restart_at(0);
    }
}
