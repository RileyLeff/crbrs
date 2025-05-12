// FILE: crbrs-lib/src/config.rs

use crate::{Error, Settings}; // Import from lib.rs
use config::{Config, File};
use directories::ProjectDirs;
use std::path::PathBuf;
use toml;

const CONFIG_FILE_NAME: &str = "config.toml";

// Helper to get project directories
pub fn get_project_dirs() -> Result<ProjectDirs, Error> {
    ProjectDirs::from("com", "YourOrg", "crbrs") // Adjust qualifier/org if desired
        .ok_or(Error::DirectoryResolutionFailed)
}

// Function to get the effective path where compilers are stored
pub fn get_compiler_storage_path(settings: &Settings) -> Result<PathBuf, Error> {
    match &settings.compiler_storage_path {
        Some(path) => Ok(path.clone()),
        None => {
            // Default to a subdirectory within the project's data directory
            let proj_dirs = get_project_dirs()?;
            Ok(proj_dirs.data_local_dir().join("compilers"))
        }
    }
}

// Function to get the path to the configuration file
pub fn get_config_file_path() -> Result<PathBuf, Error> {
    let proj_dirs = get_project_dirs()?;
    let config_dir = proj_dirs.config_dir();
    Ok(config_dir.join(CONFIG_FILE_NAME))
}


pub fn load_settings() -> Result<Settings, Error> {
    let config_file_path = get_config_file_path()?;
    let config_dir = config_file_path.parent().ok_or_else(|| Error::Io(
        std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found") // Should not happen if get_config_file_path succeeds
    ))?;

    // Ensure config directory exists (optional, depends on desired behavior)
    // std::fs::create_dir_all(config_dir)?;

    log::debug!("Attempting to load configuration from: {:?}", config_file_path);

    let settings = Config::builder()
        // Start with default values for Settings
        .add_source(Config::try_from(&Settings::default())?)
        // Layer on the user's config file if it exists
        .add_source(File::from(config_file_path.clone()).required(false))
        // TODO: Add environment variable overrides? e.g., CRBRS_WINE_PATH
        .build()?;

    log::debug!("Configuration loaded successfully.");

    // Deserialize the entire configuration hierarchy into our Settings struct
    settings.try_deserialize::<Settings>().map_err(Error::Config)
}

pub fn save_settings(settings: &Settings) -> Result<(), Error> {
    let config_file_path = get_config_file_path()?;
     let config_dir = config_file_path.parent().ok_or_else(|| Error::Io(
        std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found")
    ))?;

    // Ensure the configuration directory exists
    std::fs::create_dir_all(config_dir)?;

    // Serialize the settings into TOML format
    let toml_content = toml::to_string_pretty(settings)
        .map_err(|e| Error::Config(config::ConfigError::Foreign(Box::new(e))))?; // Wrap toml error

    // Write the TOML content to the config file
    std::fs::write(&config_file_path, toml_content)?;

    log::info!("Configuration saved to: {:?}", config_file_path);
    Ok(())
}