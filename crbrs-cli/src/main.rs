// FILE: crbrs-cli/src/main.rs

use clap::{Parser, Subcommand};
use crbrs_lib::{Error, Settings}; // Assuming Error and Settings are pub in lib
use std::path::PathBuf;
use ::config::ConfigError;

#[derive(Parser, Debug)]
#[command(author, version, about = "CRBasic Toolchain for Rustaceans", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8, // Example: Allow -v, -vv for more verbose logging
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Compile a CRBasic file
    Compile {
        /// Input CRBasic file path
        input_file: PathBuf,

        /// Optional: Output path for compiler log/info
        #[arg(short, long)]
        output_log: Option<PathBuf>,

        /// Optional: ID of the compiler to use (overrides file association)
        #[arg(short, long)]
        compiler: Option<String>,
    },
    /// Manage compilers
    Compiler {
        #[command(subcommand)]
        action: CompilerAction,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
enum CompilerAction {
    /// Install a compiler from the repository using its ID
    Install {
        /// Unique ID of the compiler from the repository manifest
        compiler_id: String,
    },
    /// List installed compilers
    List,
    /// Remove an installed compiler by its ID
    Remove {
        /// Unique ID of the compiler to remove
        compiler_id: String
    },
    // Add SetDefault later if needed, sticking to file associations for now
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show current configuration settings
    Show,
    /// Show the path to the configuration file
    Path,
    /// Set a specific configuration value (e.g., wine_path, compiler_repository_url)
    Set {
        key: String,
        value: String
    },
    /// Associate a file extension (e.g., 'cr2') with a compiler ID
    SetAssociation {
        #[arg(short, long)]
        extension: String,
        #[arg(short, long)]
        compiler_id: String,
    },
    /// Remove an association for a file extension
    UnsetAssociation {
        #[arg(short, long)]
        extension: String,
    },
}

fn main() {
    // --- 1. Parse Arguments ---
    let cli = Cli::parse();

    // --- 2. Setup Logging ---
    // Adjust log level based on verbosity flags
    let log_level = match cli.verbose {
        0 => log::LevelFilter::Warn, // Default
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    env_logger::Builder::new().filter_level(log_level).init();

    log::debug!("CLI arguments parsed: {:?}", cli);

    // --- 3. Load Settings ---
    // We need settings for most commands, so load them early.
    let settings = match crbrs_lib::config::load_settings() {
         Ok(s) => s,
         Err(e) => {
             log::error!("Failed to load settings: {}", e);
             eprintln!("Error loading configuration: {}", e); // Also print to stderr for visibility
             std::process::exit(1);
         }
     };

    // --- 4. Run Command ---
    // Pass mutable settings if commands might modify them (like install, config set)
    if let Err(e) = run_command(cli.command, settings) {
        log::error!("Command execution failed: {}", e);
        eprintln!("Error: {}", e); // Print user-facing error
        std::process::exit(1);
    }

    log::debug!("Command executed successfully.");
}

// --- Command Dispatch Logic ---
fn run_command(command: Commands, mut settings: Settings) -> Result<(), Error> {
    match command {
        Commands::Compile { input_file, output_log, compiler } => {
            log::info!("Executing Compile command...");
            // Call the placeholder function from the lib for now
            crbrs_lib::compile_file(input_file, output_log, compiler, &settings)?;
            println!("Compile command finished (placeholder)."); // User feedback
        }
        Commands::Compiler { action } => {
            match action {
                CompilerAction::Install { compiler_id } => {
                    log::info!("Executing Compiler Install command for ID: {}", compiler_id);
                    // TODO: Implement compiler installation logic in crbrs-lib
                    println!("Compiler Install command (ID: {}) finished (placeholder).", compiler_id);
                    // This action would modify settings, so we'd need crbrs_lib::config::save_settings(&settings)? here.
                }
                CompilerAction::List => {
                    log::info!("Executing Compiler List command...");
                    println!("Installed Compilers:");
                    if settings.installed_compilers.is_empty() {
                        println!("  (None)");
                    } else {
                        for (id, info) in settings.installed_compilers.iter() {
                            println!("  - ID: {}, Version: {}, Executable: {:?}", id, info.version, info.executable_name);
                        }
                    }
                }
                CompilerAction::Remove { compiler_id } => {
                    log::info!("Executing Compiler Remove command for ID: {}", compiler_id);
                    // TODO: Implement compiler removal logic in crbrs-lib
                    println!("Compiler Remove command (ID: {}) finished (placeholder).", compiler_id);
                     // This action would modify settings, so we'd need crbrs_lib::config::save_settings(&settings)? here.
                }
            }
        }
        Commands::Config { action } => {
             match action {
                ConfigAction::Show => {
                    log::info!("Executing Config Show command...");
                    // Use debug formatting for potentially sensitive paths, or format nicely
                    // println!("{:#?}", settings); // Quick dump
                    // Or format it nicely:
                    println!("Configuration Settings:");
                    println!("  Repository URL: {}", settings.compiler_repository_url);
                    let storage_path = crbrs_lib::config::get_compiler_storage_path(&settings)?; // Resolve default if needed
                    println!("  Compiler Storage Path: {}", storage_path.display());
                    println!("  Wine Path: {}", settings.wine_path.as_deref().unwrap_or("(Not Set - using PATH)"));
                    println!("  File Associations:");
                    if settings.file_associations.is_empty() {
                        println!("    (None)");
                    } else {
                        for (ext, id) in settings.file_associations.iter() {
                            println!("    .{} -> {}", ext, id);
                        }
                    }
                }
                 ConfigAction::Path => {
                    let path = crbrs_lib::config::get_config_file_path()?;
                    println!("{}", path.display());
                 }
                 ConfigAction::Set { key, value } => {
                        log::info!("Executing Config Set command (Key: {}, Value: {})", key, &value); // Log before move
                        match key.as_str() {
                            // Clone value before moving it into settings
                            "compiler_repository_url" => settings.compiler_repository_url = value.clone(),
                            // Clone value before moving it into the Option
                            "wine_path" => settings.wine_path = Some(value.clone()),
                            _ => {
                                // Error needs to be returned before the println/save
                                let err_msg = format!("Unknown configuration key: {}", key);
                                log::error!("{}", err_msg);
                                // Use the correct Error variant we defined for config issues if it's not a direct ConfigError
                                // Let's refine this - maybe a new dedicated Error variant?
                                // For now, let's use a generic message within the existing Config variant structure.
                                return Err(crbrs_lib::Error::Config(config::ConfigError::Message(err_msg)));
                            }
                        }
                        // Now 'value' itself is still valid because we moved clones
                        println!("Set '{}' = '{}'", key, value);
                        crbrs_lib::config::save_settings(&settings)?; // Save changes
                    }
                ConfigAction::SetAssociation { extension, compiler_id } => {
                    log::info!("Executing Config SetAssociation command (Ext: {}, ID: {})", extension, compiler_id);
                    let cleaned_ext = extension.trim_start_matches('.').to_lowercase();
                    if cleaned_ext.is_empty() || cleaned_ext.contains('.') {
                         return Err(Error::InvalidExtension(extension));
                    }
                    // Optional: Check if compiler_id exists in installed_compilers?
                    settings.file_associations.insert(cleaned_ext.clone(), compiler_id.clone());
                    println!("Associated '.{}' with compiler '{}'", cleaned_ext, compiler_id);
                    crbrs_lib::config::save_settings(&settings)?; // Save changes
                }
                ConfigAction::UnsetAssociation { extension } => {
                    log::info!("Executing Config UnsetAssociation command (Ext: {})", extension);
                     let cleaned_ext = extension.trim_start_matches('.').to_lowercase();
                     if settings.file_associations.remove(&cleaned_ext).is_some() {
                        println!("Removed association for '.{}'", cleaned_ext);
                        crbrs_lib::config::save_settings(&settings)?; // Save changes
                     } else {
                        println!("No association found for '.{}'", cleaned_ext);
                     }
                }
            }
        }
    }
    Ok(())
}