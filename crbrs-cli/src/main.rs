// FILE: crbrs-cli/src/main.rs

use clap::{Parser, Subcommand};
use crbrs_lib::{Error, Settings}; // Error and Settings from crbrs-lib
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "CRBasic Toolchain for Rustaceans", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
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
        compiler_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show current configuration settings
    Show,
    /// Show the path to the configuration file
    Path,
    /// Set a specific configuration value (e.g., wine_path, compiler_repository_url)
    Set { key: String, value: String },
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
    let log_level = match cli.verbose {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    // Ensure RUST_LOG isn't overriding if we want CLI flags to take precedence
    // Or, allow RUST_LOG to override by using .parse_default_env() or similar if desired.
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp_secs() // Example: Add timestamps
        .init();

    log::debug!("CLI arguments parsed: {:?}", cli);

    // --- 3. Load Settings ---
    let mut settings = match crbrs_lib::config::load_settings() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to load settings: {}", e);
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };
    log::debug!("Settings loaded successfully.");


    // --- 4. Run Command ---
    // Pass mutable settings because some commands (install, remove, config set) will modify them.
    if let Err(e) = run_command(cli.command, &mut settings) {
        log::error!("Command execution failed: {}", e);
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    log::debug!("Command executed successfully.");
}

// --- Command Dispatch Logic ---
fn run_command(command: Commands, settings: &mut Settings) -> Result<(), Error> { // settings is now mutable
    match command {
        Commands::Compile {
            input_file,
            output_log,
            compiler,
        } => {
            log::info!("Executing Compile command for file: {:?}", input_file);
            // Placeholder for actual compilation logic using crbrs_lib::compiler::compile_file
            crbrs_lib::compile_file(input_file, output_log, compiler, settings)?;
            println!("Compile command finished (placeholder).");
        }
        Commands::Compiler { action } => {
            match action {
                CompilerAction::Install { compiler_id } => {
                    log::info!("Executing Compiler Install command for ID: {}", compiler_id);
                    // Call the library function for installation
                    // This function will internally modify settings and save them.
                    crbrs_lib::installer::install_compiler(settings, &compiler_id)?;
                    println!("Compiler '{}' installation process finished.", compiler_id);
                }
                CompilerAction::List => {
                    log::info!("Executing Compiler List command...");
                    println!("Installed Compilers:");
                    if settings.installed_compilers.is_empty() {
                        println!("  (None)");
                    } else {
                        for (id, info) in settings.installed_compilers.iter() {
                            println!(
                                "  - ID: {}, Version: {}, Description: {}, Executable: {:?}, Path: {:?}",
                                id,
                                info.version,
                                info.description, // Assuming CompilerInfo has description
                                info.executable_name,
                                info.install_subdir // Show relative path for installed compiler
                            );
                        }
                    }
                }
                CompilerAction::Remove { compiler_id } => {
                    log::info!("Executing Compiler Remove command for ID: {}", compiler_id);
                    // Call the library function for removal
                    // This function will internally modify settings and save them.
                    crbrs_lib::installer::remove_compiler(settings, &compiler_id)?;
                    println!("Compiler '{}' removal process finished.", compiler_id);
                }
            }
        }
        Commands::Config { action } => {
            match action {
                ConfigAction::Show => {
                    log::info!("Executing Config Show command...");
                    println!("Configuration Settings:");
                    println!("  Repository URL: {}", settings.compiler_repository_url);
                    match crbrs_lib::config::get_compiler_storage_path(settings) {
                        Ok(storage_path) => {
                            println!("  Compiler Storage Path: {}", storage_path.display());
                        }
                        Err(e) => {
                             println!("  Compiler Storage Path: (Error resolving: {})", e);
                        }
                    }
                    println!(
                        "  Wine Path: {}",
                        settings.wine_path.as_deref().unwrap_or("(Not Set - using PATH)")
                    );
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
                    log::info!("Executing Config Set command (Key: {}, Value: {})", key, &value);
                    match key.as_str() {
                        "compiler_repository_url" => settings.compiler_repository_url = value.clone(),
                        "wine_path" => settings.wine_path = Some(value.clone()),
                        "compiler_storage_path" => settings.compiler_storage_path = Some(PathBuf::from(value.clone())),
                        _ => {
                            let err_msg = format!("Unknown configuration key: {}", key);
                            log::error!("{}", err_msg);
                            return Err(Error::Config(config::ConfigError::Message(err_msg)));
                        }
                    }
                    println!("Set '{}' = '{}'", key, value); // 'value' is still valid due to clone
                    crbrs_lib::config::save_settings(settings)?; // Save changes
                }
                ConfigAction::SetAssociation {
                    extension,
                    compiler_id,
                } => {
                    log::info!(
                        "Executing Config SetAssociation command (Ext: {}, ID: {})",
                        extension,
                        compiler_id
                    );
                    let cleaned_ext = extension.trim_start_matches('.').to_lowercase();
                    if cleaned_ext.is_empty() || cleaned_ext.contains('.') {
                        return Err(Error::InvalidExtension(extension));
                    }
                    // Optional: Check if compiler_id exists in settings.installed_compilers
                    // if !settings.installed_compilers.contains_key(&compiler_id) {
                    //     log::warn!("Associating with compiler ID '{}' which is not currently installed.", compiler_id);
                    // }
                    settings
                        .file_associations
                        .insert(cleaned_ext.clone(), compiler_id.clone());
                    println!("Associated '.{}' with compiler '{}'", cleaned_ext, compiler_id);
                    crbrs_lib::config::save_settings(settings)?;
                }
                ConfigAction::UnsetAssociation { extension } => {
                    log::info!(
                        "Executing Config UnsetAssociation command (Ext: {})",
                        extension
                    );
                    let cleaned_ext = extension.trim_start_matches('.').to_lowercase();
                    if settings.file_associations.remove(&cleaned_ext).is_some() {
                        println!("Removed association for '.{}'", cleaned_ext);
                        crbrs_lib::config::save_settings(settings)?;
                    } else {
                        println!("No association found for '.{}'", cleaned_ext);
                    }
                }
            }
        }
    }
    Ok(())
}