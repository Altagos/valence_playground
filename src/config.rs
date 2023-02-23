mod server;
mod world;

use std::{
    env,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::{Ok, Result};
use serde::{Deserialize, Serialize};

pub use self::{server::*, world::*};

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub gui: bool,
    pub world: WorldConfig,
    pub server: ServerConfig,
}

impl Default for Config {
    fn default() -> Self {
        let gui;

        cfg_if::cfg_if! {
            if #[cfg(feature = "gui")] {
                gui = true;
            } else {
                gui = false;
            }
        }

        Self {
            gui,
            world: Default::default(),
            server: Default::default(),
        }
    }
}

impl Config {
    pub fn write_toml_default(path: PathBuf) -> Result<Self> {
        let mut f = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let config = toml::to_string_pretty(&Config::default())?;
        f.write_all(config.as_bytes())?;

        Ok(Config::default())
    }

    pub fn write_ron_default(path: PathBuf) -> Result<Self> {
        let mut f = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let config =
            ron::ser::to_string_pretty(&Config::default(), ron::ser::PrettyConfig::default())?;
        f.write_all(config.as_bytes())?;

        Ok(Config::default())
    }

    pub fn write_toml(&self, path: PathBuf) -> Result<()> {
        let mut f = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let config = toml::to_string_pretty(self)?;
        f.write_all(config.as_bytes())?;

        Ok(())
    }

    pub fn write_ron(&self, path: PathBuf) -> Result<()> {
        let mut f = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let config =
            ron::ser::to_string_pretty(&Config::default(), ron::ser::PrettyConfig::default())?;
        f.write_all(config.as_bytes())?;

        Ok(())
    }

    pub fn from_toml(path: PathBuf) -> Result<Self> {
        let mut f = File::options().read(true).open(&path)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;

        Ok(toml::from_str(&buf)?)
    }

    pub fn from_ron(path: PathBuf) -> Result<Self> {
        let mut f = File::options().read(true).open(&path)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;

        Ok(ron::from_str(&buf)?)
    }

    pub fn from_current_dir_toml() -> Result<Self> {
        let current_dir = env::current_dir()?;
        let path = current_dir.join("Config.toml");

        match File::options().read(true).open(&path) {
            Result::Ok(mut f) => {
                let mut buf = String::new();
                f.read_to_string(&mut buf)?;
                Ok(toml::from_str(&buf)?)
            }
            Result::Err(_) => Config::write_toml_default(path),
        }
    }

    pub fn from_current_dir_ron() -> Result<Self> {
        let current_dir = env::current_dir()?;
        let path = current_dir.join("Config.ron");

        match File::options().read(true).open(&path) {
            Result::Ok(mut f) => {
                let mut buf = String::new();
                f.read_to_string(&mut buf)?;
                Ok(ron::from_str(&buf)?)
            }
            Result::Err(_) => Config::write_ron_default(path),
        }
    }

    pub fn from_current_dir() -> Result<Self> {
        let current_dir = env::current_dir()?;
        let path_ron = current_dir.join("Config.ron");
        let path_toml = current_dir.join("Config.toml");

        let c = match Self::from_ron(path_ron) {
            Result::Ok(c) => c,
            Result::Err(_) => match Self::from_toml(path_toml.clone()) {
                Result::Ok(c) => c,
                Result::Err(_) => Self::write_toml_default(path_toml)?,
            },
        };

        Ok(c)
    }
}
