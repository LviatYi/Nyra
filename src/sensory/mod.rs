pub mod overlay;

use crate::sensory::overlay::{drag_overlay, rotate_tips, setup_overlay};
use bevy::prelude::*;

pub struct SensoryPlugin;

impl Plugin for SensoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_overlay)
            .add_systems(Update, (rotate_tips, drag_overlay));
    }
}
