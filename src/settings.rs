use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use config::{ConfigError, Config};
use config::File as ConfigFile;
use log::debug;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub default_payout: i32,
    pub active_time: i32
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            default_payout: 1,
            active_time: 5
        }
    }
}

impl Settings {
    /// Loads the configuration or, if it doesn't exist, creates a new one filled with defaults
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();

        let config_folder = Path::new("config");
        let config_file = Path::new("config/userservice.toml");
        if !config_folder.exists() {
            debug!("Config folder does not exist, creating one");
            std::fs::create_dir(config_folder).unwrap();
        }
        if !config_file.exists() {
            debug!("Config file does not exist, creating a default config");
            let default_settings = Settings::default();
            default_settings.save().unwrap();
        }

        s.merge(ConfigFile::with_name("config/userservice"))?;
        s.try_into()
    }

    /// Saves the configuration to the file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_str = toml::to_string(&self)?;
        File::create("config/userservice.toml")?.write_all(config_str.as_bytes())?;

        return Ok(());
    }
}