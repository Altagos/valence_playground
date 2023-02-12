use bevy::prelude::*;
use bevy_inspector_egui::{
    bevy_egui::{self},
    bevy_inspector::{self},
    egui, DefaultInspectorConfigPlugin,
};

use crate::terrain::TerrainSettings;

pub struct InspectorPlugin;

impl Plugin for InspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(DefaultInspectorConfigPlugin)
            .add_system(inspector_ui);
    }
}

fn inspector_ui(world: &mut World, mut disabled: Local<bool>, mut update: Local<bool>) {
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
    let egui_context = world
        .resource_mut::<bevy_egui::EguiContext>()
        .ctx_mut()
        .clone();

    {
        if *update {
            let mut terrain_settings = world.resource_mut::<TerrainSettings>();
            terrain_settings.update = true;
        }
    }

    egui::Window::new("Terrain Settings").show(&egui_context, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            bevy_inspector::ui_for_resource::<TerrainSettings>(world, ui);

            ui.separator();
            ui.label("Press space to toggle");

            let button = ui.button("Update");

            if button.clicked() {
                button.surrender_focus();
                *update = true;
            } else {
                *update = false;
            }
        });
    });
}
