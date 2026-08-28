pub mod overlay;

use crate::sensory::overlay::{drag_overlay, rotate_tips, setup_overlay};
use bevy::prelude::*;
use crate::RotationState;

pub struct SensoryPlugin;

impl Plugin for SensoryPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<RotationState>()
            .add_systems(Startup, setup_overlay)
            .add_systems(Update, (rotate_tips, drag_overlay));
    }
}
