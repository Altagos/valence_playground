use bevy::prelude::*;
use bevy_inspector_egui::{
    bevy_egui::{self},
    DefaultInspectorConfigPlugin,
};

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(DefaultInspectorConfigPlugin)
            .add_system(inspector_ui);
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
        return;
    }

    // the usual `ResourceInspector` code
    let _egui_context = world
        .resource_mut::<bevy_egui::EguiContext>()
        .ctx_mut()
        .clone();
}
