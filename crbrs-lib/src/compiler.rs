// FILE: crbrs-lib/src/compiler.rs

use crate::{CompilerInfo, Error, Settings}; // Assuming Error and Settings are pub in lib
use std::path::{Path, PathBuf};
use std::process::{Command, Output}; // Added Output for capturing stdout/stderr

// Helper function to determine if we are likely on a non-Windows OS
// This is a compile-time check.
fn is_non_windows_os() -> bool {
    cfg!(not(windows))
}

/// Compiles a given CRBasic file using the specified or associated compiler.
pub fn compile_file_impl(
    input_file: &Path,
    output_log_param: Option<&Path>, // Changed to &Path for consistency
    compiler_id_param: Option<&str>, // Changed to &str
    settings: &Settings,
) -> Result<(), Error> {
    log::info!(
        "Attempting to compile file: {:?}, explicit compiler ID: {:?}",
        input_file,
        compiler_id_param
    );

    // 1. Validate input file
    if !input_file.exists() {
        log::error!("Input file not found: {:?}", input_file);
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Input file not found: {}", input_file.display()),
        )));
    }
    if !input_file.is_file() {
        log::error!("Input path is not a file: {:?}", input_file);
         return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput, // Or a custom error
            format!("Input path is not a file: {}", input_file.display()),
        )));
    }

    // 2. Resolve Compiler ID
    let compiler_id: String = match compiler_id_param {
        Some(id) => id.to_string(),
        None => {
            let extension = input_file
                .extension()
                .and_then(|ext| ext.to_str())
                .ok_or_else(|| Error::InvalidExtension(
                    input_file.file_name().unwrap_or_default().to_string_lossy().into_owned()
                ))?;
            settings
                .file_associations
                .get(extension)
                .cloned() // Clone the String
                .ok_or_else(|| Error::NoCompilerForExtension(extension.to_string()))?
        }
    };
    log::debug!("Resolved compiler ID to use: {}", compiler_id);

    // 3. Get CompilerInfo
    let compiler_info = settings
        .installed_compilers
        .get(&compiler_id)
        .ok_or_else(|| Error::CompilerNotFound(compiler_id.clone()))?;
    log::debug!("Using compiler info: {:?}", compiler_info);

    // 4. Construct Paths
    let compiler_base_storage_path = crate::config::get_compiler_storage_path(settings)?;
    let compiler_executable_path = compiler_base_storage_path
        .join(&compiler_info.install_subdir)
        .join(&compiler_info.executable_name);

    if !compiler_executable_path.exists() {
        log::error!("Compiler executable not found at path: {:?}", compiler_executable_path);
        return Err(Error::CompilerNotFound(format!(
            "Executable for '{}' not found at expected path: {}",
            compiler_id, compiler_executable_path.display()
        )));
    }

    // Determine output log path: use provided, or default to <input_filename>.log in same dir
    let output_log_path: PathBuf = match output_log_param {
        Some(p) => p.to_path_buf(),
        None => input_file.with_extension("log"),
    };
    log::debug!("Compiler executable: {:?}", compiler_executable_path);
    log::debug!("Input CRBasic file: {:?}", input_file);
    log::debug!("Output log file: {:?}", output_log_path);


    // 5. Prepare Command
    let mut cmd: Command;
    let mut args: Vec<String> = Vec::new(); // Store arguments separately for logging

    if compiler_info.requires_wine && is_non_windows_os() {
        let wine_exe = settings.wine_path.as_deref().unwrap_or("wine");
        cmd = Command::new(wine_exe);
        cmd.arg(compiler_executable_path.to_string_lossy().to_string()); // Wine needs compiler path as first arg
        args.push(wine_exe.to_string());
        args.push(compiler_executable_path.to_string_lossy().to_string());
        log::info!("Using Wine. Wine executable: {}", wine_exe);
    } else {
        cmd = Command::new(&compiler_executable_path);
        args.push(compiler_executable_path.to_string_lossy().to_string());
        log::info!("Running compiler natively (Windows or requires_wine=false).");
    }

    // Add compiler-specific arguments (input file, output log)
    // Assuming the standard: <compiler> <inputfile> <outputlogfile>
    let input_file_str = input_file.to_string_lossy().to_string();
    let output_log_path_str = output_log_path.to_string_lossy().to_string();

    cmd.arg(&input_file_str);
    cmd.arg(&output_log_path_str);
    args.push(input_file_str.clone());
    args.push(output_log_path_str.clone());

    log::info!("Executing command: {}", args.join(" "));

    // 6. Execute Command
    let execution_result: Result<Output, std::io::Error> = cmd.output(); // Use output() to capture stdout/stderr

    match execution_result {
        Ok(output) => {
            log::debug!("Compiler process finished.");
            log::debug!("  Status: {}", output.status);
            if !output.stdout.is_empty() {
                log::debug!("  Stdout: {}", String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                // Wine often prints benign messages to stderr.
                // We primarily care about the compiler's log file for CRBasic errors.
                log::warn!("  Stderr: {}", String::from_utf8_lossy(&output.stderr));
            }

            // 7. Handle Output/Log (after command execution)
            // Always try to read the log file, as the compiler writes status there.
            let log_content = match std::fs::read_to_string(&output_log_path) {
                Ok(content) => content,
                Err(e) => {
                    log::error!("Failed to read compiler log file {:?}: {}", output_log_path, e);
                    // If the compiler process itself failed AND we can't read its log,
                    // it's a more severe issue.
                    if !output.status.success() {
                         return Err(Error::CompilationFailed {
                            exit_code: output.status.code(),
                            stderr: format!(
                                "Compiler execution failed (Exit Code: {:?}). Additionally, failed to read log file '{}': {}",
                                output.status.code(),
                                output_log_path.display(),
                                e
                            ),
                        });
                    }
                    // If compiler seemed to succeed but log is unreadable, that's also odd.
                    return Err(Error::Io(e)); // Or a custom error
                }
            };

            log::trace!("Compiler log content:\n{}", log_content);

            // Check for success/failure based on the log content (common CRBasic compiler pattern)
            // Example patterns: "Compiled OK." or "Compile Failed!"
            if log_content.contains("Compiled OK.") { // Adjust this pattern as needed
                log::info!("Compilation successful for {:?}. Log: {:?}", input_file, output_log_path);
                println!("Compilation successful: {}", input_file.display());
                println!("Compiler log: {}", output_log_path.display());
                // Optionally print the log content or a summary
                // println!("{}", log_content);
                Ok(())
            } else if log_content.contains("Compile Failed!") { // Adjust this pattern
                log::error!("Compilation failed for {:?}. See log: {:?}", input_file, output_log_path);
                eprintln!("Compilation FAILED: {}", input_file.display());
                eprintln!("Compiler log: {}", output_log_path.display());
                // We'll return the structured error with log content.
                Err(Error::CompilationFailedWithLog { log_content })
            } else if !output.status.success() {
                // Fallback if log doesn't have clear success/fail but exit code is bad
                log::error!(
                    "Compiler process for {:?} exited with error code {:?} but log format was unrecognized.",
                    input_file,
                    output.status.code()
                );
                 Err(Error::CompilationFailed {
                    exit_code: output.status.code(),
                    stderr: format!(
                        "Compiler execution failed (Exit Code: {:?}). Log content was:\n{}",
                        output.status.code(),
                        log_content
                    ),
                })
            }
            else {
                // If exit code was success but log format is unrecognized.
                log::warn!("Compiler process for {:?} seemed to succeed (exit code 0) but log format was unrecognized. Assuming success.", input_file);
                println!("Compilation finished (unrecognized log format, but process exited successfully): {}", input_file.display());
                println!("Compiler log: {}", output_log_path.display());
                Ok(())
            }
        }
        Err(e) => {
            // This error means `cmd.output()` itself failed (e.g., wine not found, compiler exe not found by OS)
            log::error!("Failed to execute compiler command: {}", e);
            // Check if it's because wine was not found (specifically for non-Windows)
            if compiler_info.requires_wine && is_non_windows_os() && e.kind() == std::io::ErrorKind::NotFound {
                return Err(Error::WineNotFound);
            }
            Err(Error::Subprocess(e))
        }
    }
}