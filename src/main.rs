#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]

use std::io;

use bevy::prelude::App;
use chrono::Local;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use valence_playground::{gui::GuiPlugin, minecraft::MinecraftPlugin};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    if let Ok(path) = std::env::var("RUST_LOG_PATH") {
        let appender = tracing_appender::rolling::never(
            &path,
            format!("{}.log", Local::now().format("%d.%m.%Y_%H:%M:%S")),
        );
        let (non_blocking, _guard) = tracing_appender::non_blocking(appender);

        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(fmt::layer().with_writer(io::stdout))
            .with(
                fmt::layer()
                    .with_writer(non_blocking)
                    .compact()
                    .with_ansi(false),
            )
            .init();

        tracing::info!("Logfiles are located at: {path}");

        App::new()
            .add_plugin(MinecraftPlugin)
            .add_plugin(GuiPlugin)
            .run();
    } else {
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(fmt::layer().with_writer(io::stdout))
            .init();

        App::new()
            .add_plugin(MinecraftPlugin)
            .add_plugin(GuiPlugin)
            .run();
    }
}
