// FILE: crbrs-lib/src/lib.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use ::config::ConfigError;

// --- Configuration Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)] // Clone is useful for modifying settings
#[serde(default)] // Ensure defaults are used if fields are missing in config file
pub struct Settings {
    pub compiler_repository_url: String,
    pub compiler_storage_path: Option<PathBuf>, // Option allows finding default if None
    pub installed_compilers: HashMap<String, CompilerInfo>,
    pub file_associations: HashMap<String, String>, // Key: extension (e.g., "cr2"), Value: compiler ID
    pub wine_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            // TODO: Consider a more permanent default URL later
            compiler_repository_url: "https://example.com/compilers.toml".to_string(),
            compiler_storage_path: None, // We'll resolve this to a default path at runtime
            installed_compilers: HashMap::new(),
            file_associations: HashMap::new(),
            wine_path: None, // Will try finding 'wine' in PATH by default
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompilerInfo {
    pub id: String,
    pub description: String, // Added description for user clarity
    pub version: String,
    // Relative path to the compiler *directory* within the storage path
    pub install_subdir: PathBuf,
    pub executable_name: String,
    #[serde(default = "default_true")] // Assume requires wine unless specified otherwise
    pub requires_wine: bool,
    #[serde(default)] // Optional field
    pub supported_loggers: Option<Vec<String>>,
    // We might add download_url and sha256 here if we want to store the manifest info locally
    // after install, but let's keep it simple for now.
}

// Helper for serde default
fn default_true() -> bool {
    true
}

// --- Error Enum ---

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Failed to process ZIP archive: {0}")]
    Zip(#[from] zip::result::ZipError),

    // #[error("Failed to process TAR archive: {0}")] // Add if/when tar support is added
    // Tar(#[from] tar::Error), // Requires tar crate

    #[error("Compiler '{0}' not found in configuration.")]
    CompilerNotFound(String),

    #[error("No compiler associated with file extension '.{0}'. Please configure an association.")]
    NoCompilerForExtension(String),

    #[error("Could not find Wine executable. Please install Wine or set the path in configuration.")]
    WineNotFound,

    #[error("Failed to execute subprocess: {0}")]
    Subprocess(std::io::Error), // Separate from general Io for clarity

    #[error("Compiler execution failed (Exit Code: {exit_code:?}). Stderr:\n{stderr}")]
    CompilationFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("Compiler execution failed. Output Log:\n{log_content}")]
    CompilationFailedWithLog { log_content: String }, // Use if we parse the log

    #[error("Invalid compiler source or manifest: {0}")]
    InvalidCompilerSource(String),

    #[error("Failed to determine application directories.")]
    DirectoryResolutionFailed,

    #[error("Compiler ID '{0}' not found in the repository manifest.")]
    CompilerIdNotFoundInManifest(String),

    #[error("Invalid file extension: '{0}'.")]
    InvalidExtension(String),
}

// Define pub modules for organization (create the files next)
pub mod config;
pub mod compiler;
// pub mod download; // Maybe later

// Example function signature using the types (implementation later)
pub fn compile_file(
    input_file: PathBuf,
    output_log: Option<PathBuf>,
    compiler_id: Option<String>,
    settings: &Settings,
) -> Result<(), Error> {
    // Placeholder - actual logic will go into compiler.rs
    println!(
        "Placeholder: Compiling {:?} using compiler {:?} with settings.",
        input_file, compiler_id
    );
    // Find compiler executable path based on compiler_id or file extension + settings.file_associations
    // Determine if Wine is needed
    // Run std::process::Command (potentially via Wine)
    // Check exit code and stderr/stdout
    // Parse output_log if provided and successful/failed
    Ok(())
}


#[cfg(test)]
mod tests {
    // Add basic tests later if needed
}