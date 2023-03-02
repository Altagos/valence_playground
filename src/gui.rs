use bevy::prelude::*;

pub mod inspector;

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    #[allow(unused_variables)]
    fn build(&self, app: &mut bevy::prelude::App) {
        #[cfg(feature = "gui")]
        {
            use crate::CONFIG;

            if CONFIG.gui {
                use bevy::{log::LogPlugin, window::PresentMode};
                use bevy_inspector_egui::bevy_egui::EguiPlugin;

                use self::inspector::InspectorPlugin;
                use crate::minecraft::chat::gui_chat_window;

                app.add_plugins(
                    DefaultPlugins
                        .set(WindowPlugin {
                            window: WindowDescriptor {
                                title: "Valence Playground".to_string(),
                                width: 600.,
                                height: 600.,
                                present_mode: PresentMode::AutoVsync,
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .disable::<LogPlugin>(),
                )
                .add_plugin(EguiPlugin)
                .add_plugin(InspectorPlugin)
                .add_startup_system(setup_camera)
                .add_system(gui_chat_window);
            }
        }
    }
}

#[allow(dead_code)]
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}
