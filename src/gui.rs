use bevy::prelude::*;

pub mod inspector;

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    #[allow(unused_variables)]
    fn build(&self, app: &mut bevy::prelude::App) {
        #[cfg(feature = "gui")]
        {
            use bevy::window::PresentMode;
            use bevy_inspector_egui::bevy_egui::EguiPlugin;

            use self::inspector::InspectorPlugin;
            use crate::chat::gui_chat_window;

            app.add_plugins(DefaultPlugins.set(WindowPlugin {
                window: WindowDescriptor {
                    title: "Valence Playground".to_string(),
                    width: 600.,
                    height: 600.,
                    present_mode: PresentMode::AutoVsync,
                    ..Default::default()
                },
                ..Default::default()
            }))
            .add_plugin(EguiPlugin)
            .add_plugin(InspectorPlugin)
            .add_system(gui_chat_window);
        }

        #[cfg(not(feature = "gui"))]
        {
            use tracing_subscriber::EnvFilter;

            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .init();
        }
    }
}