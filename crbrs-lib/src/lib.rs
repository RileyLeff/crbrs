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
            compiler_repository_url: "https://raw.githubusercontent.com/RileyLeff/campbell-scientific-compilers/refs/heads/main/compilers.toml".to_string(),
            compiler_storage_path: None, // We'll resolve this to a default path at runtime
            installed_compilers: HashMap::new(),
            file_associations: HashMap::new(),
            wine_path: None, // Will try finding 'wine' in PATH by default
        }
    }
}

// --- CompilerInfo (Installed Compiler) - Slight Refinement ---
// This struct represents a compiler *after* it has been installed.
// It might store slightly different or additional info compared to the manifest entry.

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompilerInfo {
    pub id: String,                 // e.g., "cr2comp-v4.0" (matches manifest key)
    pub description: String,        // From manifest
    pub version: String,            // From manifest
    pub install_subdir: PathBuf,    // Path to the *directory* of this compiler relative to compiler_storage_path
                                    // (e.g., "cr2comp-v4.0/")
    pub executable_name: String,    // e.g., "cr2comp.exe" (relative to install_subdir)
    pub requires_wine: bool,        // From manifest
    pub supported_loggers: Option<Vec<String>>, // From manifest
}

// Helper for serde default
fn default_true() -> bool {
    true
}


// --- Manifest Structures (NEW) ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Manifest {
    pub manifest_version: String,
    #[serde(default)] // Allow empty 'compilers' table in TOML
    pub compilers: HashMap<String, ManifestCompilerEntry>, // Key is the Compiler ID
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManifestCompilerEntry {
    pub description: String,
    pub version: String,
    pub download_url: String,
    pub executable_name: String, // e.g., "cr2comp.exe"
    #[serde(default = "default_true")] // Assume requires wine if not specified
    pub requires_wine: bool,
    #[serde(default)]
    pub supported_loggers: Option<Vec<String>>,
    #[serde(default)]
    pub sha256: Option<String>, // Optional checksum for verification
}

#[derive(Debug, Clone)] // Clone might be useful
pub struct CompilationErrorDetail {
    pub file_path_in_log: String, // e.g., "example.cr2" from the log's first line
    pub line: Option<u32>,
    pub message: String,
    // Add character_pos or other fields if the compiler ever provides them
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

    #[error("Compilation of '{file_path}' failed.")]
    CompilationFailed {
        file_path: PathBuf, // The input file path
        errors: Vec<CompilationErrorDetail>,
        raw_log: String, // Always include the full log for reference
    },

    // Keep a simpler variant if log parsing itself fails or is ambiguous
    #[error("Compiler reported failure for '{file_path}'. Log:\n{raw_log}")]
    GenericCompilationFailedWithLog {
        file_path: PathBuf,
        raw_log: String,
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
pub mod installer;
// pub mod download; // Maybe later

// Example function signature using the types (implementation later)
pub fn compile_file(
    input_file: PathBuf,
    output_log: Option<PathBuf>,
    compiler_id: Option<String>,
    settings: &Settings,
) -> Result<(), Error> {
    compiler::compile_file_impl(
            &input_file,
            output_log.as_deref(), // Convert Option<PathBuf> to Option<&Path>
            compiler_id.as_deref(), // Convert Option<String> to Option<&str>
            settings,
        )
}


#[cfg(test)]
mod tests {
    // Add basic tests later if needed
}