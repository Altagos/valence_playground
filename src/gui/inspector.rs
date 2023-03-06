use bevy::prelude::*;

use crate::minecraft::world_gen::inspector_ui as terrain_ui;

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) { app.add_system(terrain_ui); }
}
