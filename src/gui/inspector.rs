use bevy::prelude::*;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;

use crate::minecraft::world_gen::inspector_ui as terrain_ui;

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(DefaultInspectorConfigPlugin)
            .add_system(inspector_ui)
            .add_system(terrain_ui);
    }
}

fn inspector_ui(world: &mut World, mut disabled: Local<bool>) {
    let space_pressed = world
        .resource::<Input<KeyCode>>()
        .just_pressed(KeyCode::Space);
    if space_pressed {
        *disabled = !*disabled;
    }
    if *disabled {
        // noting at the moment
    }
}
