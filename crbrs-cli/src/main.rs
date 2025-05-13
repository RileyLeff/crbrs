// FILE: crbrs-cli/src/main.rs

use clap::{Parser, Subcommand};
use crbrs_lib::{Error, Settings, CompilationErrorDetail}; // Import CompilationErrorDetail
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
        compiler_id: String,
    },
    /// List *installed* compilers
    List,
    /// List *available* compilers from the remote repository
    ListAvailable,
    /// Remove an installed compiler by its ID
    Remove {
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
    let cli = Cli::parse();
    let log_level = match cli.verbose {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();

    log::debug!("CLI arguments parsed: {:?}", cli);

    let mut settings = match crbrs_lib::config::load_settings() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to load settings: {}", e);
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };
    log::debug!("Settings loaded successfully.");

    if let Err(e) = run_command(cli.command, &mut settings) {
        // Specific handling for structured compilation errors is now inside the Compile arm
        // Generic error printing for all other unhandled errors from run_command
        if !matches!(e, Error::CompilationFailed {..} | Error::GenericCompilationFailedWithLog {..}) {
             log::error!("Command execution failed: {}", e); // Log the full error
        }
        // The error message for compilation failures is printed within the Compile arm.
        // For other errors, the Display impl of the error enum will be used here.
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    log::debug!("Command executed successfully.");
}
// FILE: crbrs-cli/src/main.rs
// ... (imports, structs, main function - same as previous complete main.rs) ...

fn run_command(command: Commands, settings: &mut Settings) -> Result<(), Error> { // Error is crbrs_lib::Error
    match command {
        Commands::Compile {
            input_file,
            output_log,
            compiler,
        } => {
            log::info!("Executing Compile command for file: {:?}", input_file);
            match crbrs_lib::compile_file(input_file.clone(), output_log, compiler, settings) {
                Ok(_) => {
                    // Success messages are printed by the library function
                }
                Err(e) => {
                    match &e {
                        Error::CompilationFailed { file_path, errors, raw_log } => {
                            log::error!(
                                "CompilationFailed for {}: {:?}, Raw Log: '{}'",
                                file_path.display(), errors, raw_log
                            );
                            eprintln!("Error: Compilation of '{}' failed.", file_path.display());
                            if errors.is_empty() {
                                eprintln!("Compiler reported failure. Full log output:");
                                eprintln!("--------------------------------------------------");
                                eprintln!("{}", raw_log.trim());
                                eprintln!("--------------------------------------------------");
                            } else {
                                eprintln!("Specific errors found:");
                                for detail in errors {
                                    if !detail.file_path_in_log.is_empty() && detail.file_path_in_log != input_file.file_name().unwrap_or_default().to_string_lossy() {
                                        eprintln!("  In file (from log): {}", detail.file_path_in_log);
                                    }
                                    if let Some(line_num) = detail.line {
                                        eprintln!("  Line {}: {}", line_num, detail.message);
                                    } else {
                                        eprintln!("  Error: {}", detail.message);
                                    }
                                }
                                eprintln!("\n(See full compiler log in the .log file for more details)");
                            }
                        }
                        Error::GenericCompilationFailedWithLog { file_path, raw_log } => {
                            log::error!(
                                "GenericCompilationFailedWithLog for {}: Raw Log: '{}'",
                                file_path.display(), raw_log
                            );
                            eprintln!("Error: Compilation of '{}' failed (generic).", file_path.display());
                            eprintln!("Full log output:");
                            eprintln!("--------------------------------------------------");
                            eprintln!("{}", raw_log.trim());
                            eprintln!("--------------------------------------------------");
                        }
                        _ => {
                            log::error!("Compile command encountered an error: {}", e);
                        }
                    }
                    return Err(e);
                }
            }
        }
        Commands::Compiler { action } => {
            match action {
                CompilerAction::Install { compiler_id } => {
                    log::info!("Executing Compiler Install command for ID: {}", compiler_id);
                    crbrs_lib::installer::install_compiler(settings, &compiler_id)?;
                    println!("Compiler '{}' installation process finished.", compiler_id);
                }
                CompilerAction::List => {
                    log::info!("Executing Compiler List command...");
                    println!("Installed Compilers (Locally):");
                    if settings.installed_compilers.is_empty() {
                        println!("  (None)");
                    } else {
                        for (id, info) in settings.installed_compilers.iter() {
                            println!(
                                "  - ID: {}, Version: {}, Description: {}, Path: {:?}",
                                id, info.version, info.description, info.install_subdir
                            );
                        }
                    }
                }
                CompilerAction::ListAvailable => {
                    log::info!("Executing Compiler ListAvailable command...");
                    println!(
                        "Fetching available compilers from: {}",
                        settings.compiler_repository_url
                    );
                    match crbrs_lib::installer::fetch_manifest(&settings.compiler_repository_url) {
                        Ok(manifest) => {
                            println!("Available Compilers (Remote - Manifest Version: {}):", manifest.manifest_version);
                            if manifest.compilers.is_empty() {
                                println!("  (None found in manifest)");
                            } else {
                                let mut sorted_compilers: Vec<_> = manifest.compilers.iter().collect();
                                sorted_compilers.sort_by(|(id_a, _), (id_b, _)| id_a.cmp(id_b));
                                for (id, entry) in sorted_compilers {
                                    println!(
                                        "  - ID: {:<30} Version: {:<15} Description: {}",
                                        id, entry.version, entry.description
                                    );
                                }
                            }
                        }
                        Err(e) => { log::error!("Failed to fetch or parse remote manifest: {}", e); return Err(e); }
                    }
                }
                CompilerAction::Remove { compiler_id } => {
                    log::info!("Executing Compiler Remove command for ID: {}", compiler_id);
                    crbrs_lib::installer::remove_compiler(settings, &compiler_id)?;
                    println!("Compiler '{}' removal process finished.", compiler_id);
                }
            }
        }
        Commands::Config { action } => { // <<< --- FILLED IN SECTION --- >>>
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
                        let mut sorted_associations: Vec<_> = settings.file_associations.iter().collect();
                        sorted_associations.sort_by_key(|(ext, _)| *ext); // Sort by extension
                        for (ext, id) in sorted_associations {
                            println!("    .{} -> {}", ext, id);
                        }
                    }
                }
                ConfigAction::Path => {
                    let path = crbrs_lib::config::get_config_file_path()?;
                    println!("{}", path.display());
                }
                ConfigAction::Set { key, value } => {
                    log::info!("Executing Config Set command (Key: '{}', Value: '{}')", key, &value);
                    match key.as_str() {
                        "compiler_repository_url" => settings.compiler_repository_url = value.clone(),
                        "wine_path" => settings.wine_path = Some(value.clone()),
                        "compiler_storage_path" => settings.compiler_storage_path = Some(PathBuf::from(value.clone())),
                        _ => {
                            let err_msg = format!("Unknown configuration key: {}", key);
                            log::error!("{}", err_msg);
                            // Assuming crbrs_lib::Error::Config wraps config::ConfigError
                            // and config::ConfigError::Message is a valid way to create it
                            return Err(Error::Config(config::ConfigError::Message(err_msg)));
                        }
                    }
                    println!("Set '{}' = '{}'", key, value); // 'value' is still valid due to clone
                    crbrs_lib::config::save_settings(settings)?;
                }
                ConfigAction::SetAssociation {
                    extension,
                    compiler_id,
                } => {
                    let cleaned_ext = extension.trim_start_matches('.').to_lowercase();
                    log::info!(
                        "Executing Config SetAssociation command (Ext: .{}, ID: {})",
                        cleaned_ext,
                        compiler_id
                    );
                    if cleaned_ext.is_empty() || cleaned_ext.contains('.') {
                        return Err(Error::InvalidExtension(extension));
                    }
                    settings
                        .file_associations
                        .insert(cleaned_ext.clone(), compiler_id.clone());
                    println!("Associated '.{}' with compiler '{}'", cleaned_ext, compiler_id);
                    crbrs_lib::config::save_settings(settings)?;
                }
                ConfigAction::UnsetAssociation { extension } => {
                    let cleaned_ext = extension.trim_start_matches('.').to_lowercase();
                    log::info!(
                        "Executing Config UnsetAssociation command (Ext: .{})",
                        cleaned_ext
                    );
                    if settings.file_associations.remove(&cleaned_ext).is_some() {
                        println!("Removed association for '.{}'", cleaned_ext);
                        crbrs_lib::config::save_settings(settings)?;
                    } else {
                        println!("No association found for '.{}'", cleaned_ext);
                    }
                }
            }
        } // <<< --- END OF FILLED IN SECTION --- >>>
    }
    Ok(())
}