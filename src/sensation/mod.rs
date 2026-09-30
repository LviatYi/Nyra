pub mod tip_overlay;

use bevy::prelude::*;

pub struct SensoryPlugin;

impl Plugin for SensoryPlugin {
    fn build(&self, app: &mut App) {
        tip_overlay::configure(app);
    }
}
