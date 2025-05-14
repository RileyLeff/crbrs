// FILE: crbrs-cli/src/main.rs

use clap::{Parser, Subcommand};
use crbrs_lib::{Error, Settings, CompilationErrorDetail}; // Ensure CompilationErrorDetail is imported
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, name = "crbrs", about = "CRBasic Toolchain for Rustaceans", long_about = None)]
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
        /// Optional: Output path for compiler log/info (compiler writes to this file)
        #[arg(long)] // Changed from short 'o' to avoid conflict if we add other short flags
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
    /// Set a specific configuration value
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
        0 => log::LevelFilter::Error, // Default: Show only CRITICAL errors from our code.
        1 => log::LevelFilter::Warn,  // -v: Show WARN (like Wine stderr) and ERROR.
        2 => log::LevelFilter::Info,  // -vv: Show INFO, WARN, ERROR.
        3 => log::LevelFilter::Debug, // -vvv: Show DEBUG, INFO, WARN, ERROR.
        _ => log::LevelFilter::Trace, // -vvvv or more: Show TRACE and all above.
    };
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp(None)      // Cleaner error logs if they appear by default
        .format_module_path(false) //
        .format_target(false)      //
        .init();

    log::debug!("CLI arguments parsed: {:?}. Effective log level: {}", cli, log_level);

    let mut settings = match crbrs_lib::config::load_settings() {
        Ok(s) => {
            log::debug!("Settings loaded successfully: {:?}", s);
            s
        }
        Err(e) => {
            log::error!("Critical error loading settings: {}", e); // Shows at default Error level
            eprintln!("Error: Could not load configuration: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = run_command(cli.command, &mut settings) {
        // Log the full error detail if verbosity allows (or if it's an ERROR level log)
        // The specific user-facing `eprintln!` for compilation errors is handled in `run_command`.
        // This `log::error!` ensures other types of errors from `run_command` are logged.
        if !matches!(e, Error::CompilationFailed {..} | Error::GenericCompilationFailedWithLog {..}) {
            log::error!("Command execution failed with error: {}", e);
        }


        // Print user-facing message ONLY IF it wasn't a compilation error
        // (because those are handled with more detail in the Compile arm of run_command).
        match e {
            Error::CompilationFailed { .. } | Error::GenericCompilationFailedWithLog { .. } => {
                // User-facing message already printed by the Compile arm's error handler.
            }
            _ => {
                // For all other error types, print their Display message to the user.
                eprintln!("Error: {}", e);
            }
        }
        std::process::exit(1);
    }

    log::debug!("Command executed successfully.");
}

fn run_command(command: Commands, settings: &mut Settings) -> Result<(), Error> {
    match command {
        Commands::Compile {
            input_file,
            output_log, // This is Option<PathBuf> from clap
            compiler,
        } => {
            log::info!("Executing Compile command for file: {:?}", input_file); // Shows with -vv
            match crbrs_lib::compile_file(input_file.clone(), output_log.clone(), compiler, settings) {
                Ok(_) => {
                    // Success messages (like ✅) are printed by the library function directly.
                }
                Err(e) => {
                    // This is where we print user-facing messages for COMPILATION errors.
                    match &e {
                        Error::CompilationFailed { file_path, errors, raw_log } => {
                            eprintln!("\n❌ Compilation of '{}' failed.", file_path.display());
                            if errors.is_empty() {
                                eprintln!("  Compiler reported errors, but no specific error lines were parsed.");
                                eprintln!("  (Use '-v' with `crbrs compile` to see raw compiler stdout/stderr)");
                                log::error!(
                                    "CompilationFailed for {} but no structured errors parsed. Raw output (stdout from compiler):\n{}",
                                    file_path.display(), raw_log
                                );
                            } else {
                                eprintln!("Specific errors found:");
                                for detail in errors {
                                    if let Some(line_num) = detail.line {
                                        eprintln!("  Line {}: {}", line_num, detail.message.trim());
                                    } else {
                                        eprintln!("  Error: {}", detail.message.trim());
                                    }
                                }
                            }
                            if let Some(log_p) = output_log { // User explicitly asked for a log file
                                 eprintln!("\n(Full compiler log also available in '{}')", log_p.display());
                            } else { // Default case: no log file created by crbrs
                                 eprintln!("\n(Use '-v' with `crbrs compile` to see raw compiler output, or use --output-log to save the compiler's log)");
                            }
                        }
                        Error::GenericCompilationFailedWithLog { file_path, raw_log } => {
                            eprintln!("\n❌ Compilation of '{}' failed (compiler output format unrecognized or process error).", file_path.display());
                            eprintln!("Raw compiler output:");
                            eprintln!("--------------------------------------------------");
                            eprintln!("{}", raw_log.trim());
                            eprintln!("--------------------------------------------------");
                            if let Some(log_p) = output_log {
                                 eprintln!("\n(Full compiler log also available in '{}')", log_p.display());
                            }
                        }
                        _ => {
                            // Other errors (like WineNotFound, IoError before compilation attempt, etc.)
                            // will be logged by main's log::error! and their Display message printed by main's eprintln!
                            // We just log them here if they reached this point from the compile_file call.
                            log::error!("Compile command failed with an unexpected library error: {}", e);
                        }
                    }
                    return Err(e); // Propagate the original error to be caught by main's handler for exit code
                }
            }
        }
        Commands::Compiler { action } => {
            match action {
                CompilerAction::Install { compiler_id } => {
                    log::info!("Executing Compiler Install command for ID: {}", compiler_id);
                    crbrs_lib::installer::install_compiler(settings, &compiler_id)?;
                    println!("✅ Compiler '{}' installed successfully.", compiler_id);
                }
                CompilerAction::List => {
                    log::info!("Executing Compiler List command...");
                    println!("Installed Compilers (Locally):");
                    if settings.installed_compilers.is_empty() {
                        println!("  (None)");
                    } else {
                        let mut sorted_compilers: Vec<_> = settings.installed_compilers.values().collect();
                        sorted_compilers.sort_by_key(|info| &info.id);
                        for info in sorted_compilers {
                            println!(
                                "  - ID: {:<30} Version: {:<15} Description: {}",
                                info.id, info.version, info.description,
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
                    println!("🗑️ Compiler '{}' removed successfully.", compiler_id);
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
                        let mut sorted_associations: Vec<_> = settings.file_associations.iter().collect();
                        sorted_associations.sort_by_key(|(ext, _)| *ext);
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
                            // log::error!("{}", err_msg); // Already logged by main's catch-all
                            return Err(Error::Config(config::ConfigError::Message(err_msg)));
                        }
                    }
                    println!("Set '{}' = '{}'", key, value);
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
        }
    }
    Ok(())
}