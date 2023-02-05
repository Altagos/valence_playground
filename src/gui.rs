use bevy::prelude::Plugin;

pub mod inspector;

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    #[allow(unused_variables)]
    fn build(&self, app: &mut bevy::prelude::App) {
        #[cfg(feature = "gui")]
        {
            use bevy::prelude::DefaultPlugins;
            use bevy_inspector_egui::bevy_egui::EguiPlugin;

            use self::inspector::InspectorPlugin;

            app.add_plugins(DefaultPlugins)
                .add_plugin(EguiPlugin)
                .add_plugin(InspectorPlugin);
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
