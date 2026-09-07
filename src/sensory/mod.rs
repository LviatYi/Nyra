mod job;
pub mod tip_overlay;

use crate::sensory::job::JobSensoryState;
use bevy::prelude::*;

pub struct SensoryPlugin;

impl Plugin for SensoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JobSensoryState>();
        tip_overlay::configure(app);
    }
}
