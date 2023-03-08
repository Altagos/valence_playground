use bevy::{prelude::*, window::CompositeAlphaMode};

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
                use bevy_egui::EguiPlugin;

                use self::inspector::InspectorPlugin;
                use crate::minecraft::chat::gui_chat_window;

                app.insert_resource(ClearColor(Color::rgba(0.3, 0.3, 0.3, 0.75)))
                    .add_plugins(
                        DefaultPlugins
                            .set(WindowPlugin {
                                primary_window: Some(Window {
                                    title: "Valence Playground".to_string(),
                                    present_mode: PresentMode::AutoVsync,
                                    transparent: true,
                                    // decorations: false,
                                    #[cfg(target_os = "macos")]
                                    composite_alpha_mode: CompositeAlphaMode::PostMultiplied,
                                    ..Default::default()
                                }),
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
fn setup_camera(mut commands: Commands) { commands.spawn(Camera2dBundle::default()); }
