// FILE: crbrs_lib/src/compiler.rs

use crate::{CompilationErrorDetail, Error, Settings};
use regex::Regex;
// std::fs is not explicitly needed here anymore unless we were to do something
// special with the user-requested log file path before passing it to the compiler.
use std::path::Path;
use std::process::{Command, Output};

// Helper function to determine if we are likely on a non-Windows OS
// This is a compile-time check.
fn is_non_windows_os() -> bool {
    cfg!(not(windows))
}

/// Parses the compiler's output (typically stdout) to extract structured error details.
fn parse_compiler_output(output_content: &str) -> Result<Vec<CompilationErrorDetail>, ()> {
    let mut errors = Vec::new();
    let mut lines = output_content.lines();

    // First line usually indicates overall status and the filename as seen by compiler
    let first_line = lines.next().unwrap_or("").trim();
    let file_path_in_log = first_line // Renaming variable for clarity, though it's from stdout now
        .split(" -- ")
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    if first_line.contains("Compiled OK.") {
        return Ok(Vec::new()); // Success, no errors
    } else if first_line.contains("Compile Failed!") {
        // Regex to capture "line <number>: <message>"
        let re = Regex::new(r"^\s*line\s+(\d+):\s*(.+?)\s*$").expect("Invalid regex pattern");

        for line_str in lines {
            let trimmed_line = line_str.trim();
            if trimmed_line.is_empty() {
                continue;
            }
            if let Some(caps) = re.captures(trimmed_line) {
                let line_num_str = caps.get(1).map_or("", |m| m.as_str());
                let line_num = line_num_str.parse::<u32>().ok();
                let message = caps.get(2).map_or("", |m| m.as_str()).to_string();
                errors.push(CompilationErrorDetail {
                    file_path_in_log: file_path_in_log.clone(),
                    line: line_num,
                    message,
                });
            } else if !errors.is_empty() && !trimmed_line.starts_with("line ") {
                // If we have started parsing errors and this line doesn't look like a new error,
                // append it to the message of the last parsed error (for multi-line error messages).
                if let Some(last_error) = errors.last_mut() {
                    last_error.message.push_str("\n"); // Add a newline separator
                    last_error.message.push_str(trimmed_line); // Append the current line
                }
            }
        }
        return Ok(errors); // Contains parsed errors, or empty if only "Compile Failed!" was found
    }

    Err(()) // Unrecognized output format (neither "Compiled OK." nor "Compile Failed!" in first line)
}

/// Compiles a given CRBasic file using the specified or associated compiler.
pub fn compile_file_impl(
    input_file: &Path,
    output_log_param: Option<&Path>, // Path for the compiler's log file, if user requested one
    compiler_id_param: Option<&str>,
    settings: &Settings,
) -> Result<(), Error> {
    log::info!(
        "Attempting to compile file: {:?}, explicit compiler ID: {:?}, user-requested log: {:?}",
        input_file,
        compiler_id_param,
        output_log_param
    );

    // 1. Validate input file
    if !input_file.exists() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Input file not found: {}", input_file.display()),
        )));
    }
    if !input_file.is_file() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
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
                .ok_or_else(|| {
                    Error::InvalidExtension(
                        input_file
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    )
                })?;
            settings
                .file_associations
                .get(extension)
                .cloned()
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

    // 4. Construct Path to Compiler Executable
    let compiler_base_storage_path = crate::config::get_compiler_storage_path(settings)?;
    let compiler_executable_path = compiler_base_storage_path
        .join(&compiler_info.install_subdir)
        .join(&compiler_info.executable_name);

    if !compiler_executable_path.exists() {
        return Err(Error::CompilerNotFound(format!(
            "Executable for '{}' not found at expected path: {}",
            compiler_id,
            compiler_executable_path.display()
        )));
    }
    log::debug!("Compiler executable: {:?}", compiler_executable_path);
    log::debug!("Input CRBasic file: {:?}", input_file);

    // 5. Prepare Command
    let mut cmd: Command;
    let mut args_for_logging: Vec<String> = Vec::new();

    if compiler_info.requires_wine && is_non_windows_os() {
        let wine_exe = settings.wine_path.as_deref().unwrap_or("wine");
        cmd = Command::new(wine_exe);
        cmd.arg(compiler_executable_path.to_string_lossy().as_ref()); // Compiler path is arg to wine
        args_for_logging.push(wine_exe.to_string());
        args_for_logging.push(compiler_executable_path.to_string_lossy().into_owned());
        log::info!("Using Wine. Wine executable: {}", wine_exe);
    } else {
        cmd = Command::new(&compiler_executable_path);
        args_for_logging.push(compiler_executable_path.to_string_lossy().into_owned());
        log::info!("Running compiler natively (Windows or requires_wine=false).");
    }

    // Add input file argument
    let input_file_str = input_file.to_string_lossy().into_owned();
    cmd.arg(&input_file_str);
    args_for_logging.push(input_file_str.clone());

    // Add output log file argument ONLY if user specified one
    if let Some(log_path) = output_log_param {
        let output_log_path_str = log_path.to_string_lossy().into_owned();
        cmd.arg(&output_log_path_str);
        args_for_logging.push(output_log_path_str);
        log::debug!("Compiler will create log at user-specified path: {:?}", log_path);
    } else {
        log::debug!("Compiler will output to stdout/stderr (no explicit log file argument passed).");
    }

    log::info!("Executing command: {}", args_for_logging.join(" "));

    // 6. Execute Command
    let execution_result: Result<Output, std::io::Error> = cmd.output(); // Captures stdout, stderr, status

    match execution_result {
        Ok(output) => {
            log::debug!("Compiler process finished. Status: {}", output.status);
            let stdout_content = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr_content = String::from_utf8_lossy(&output.stderr).into_owned();

            if !stdout_content.trim().is_empty() {
                log::debug!("  Stdout from compiler process:\n{}", stdout_content.trim());
            }
            if !stderr_content.trim().is_empty() {
                // Benign Wine messages often appear here.
                log::warn!("  Stderr from compiler process:\n{}", stderr_content.trim());
            }

            // 7. Parse stdout for success/failure and errors
            match parse_compiler_output(&stdout_content) {
                Ok(parsed_errors) => { // Successfully parsed stdout (found "OK" or "Failed")
                    if parsed_errors.is_empty() { // Implies "Compiled OK."
                        log::info!("Compilation successful for {:?}.", input_file);
                        println!("✅ Successfully compiled: {}", input_file.display());
                        if let Some(log_p) = output_log_param {
                            println!("   Compiler log created at: {}", log_p.display());
                        }
                        Ok(())
                    } else { // Implies "Compile Failed!" and errors were parsed (or vec is empty but still a fail)
                        log::error!("Compilation failed for {:?} based on stdout parsing.", input_file);
                        Err(Error::CompilationFailed {
                            file_path: input_file.to_path_buf(),
                            errors: parsed_errors,
                            raw_log: stdout_content, // The stdout is the primary "log" here
                        })
                    }
                }
                Err(_) => { // parse_compiler_output returned Err -> unrecognized stdout format
                    log::warn!(
                        "Unrecognized compiler stdout format for {:?}. Relying on process exit status.",
                        input_file
                    );
                    if output.status.success() {
                        // If stdout is weird but exit code is 0, assume success but warn user.
                        log::info!(
                            "Compiler process for {:?} exited successfully despite unrecognized stdout. Assuming success.",
                            input_file
                        );
                        println!(
                            "✅ Compilation process for {} finished successfully (exit code 0), but output format was unrecognized.",
                            input_file.display()
                        );
                        if let Some(log_p) = output_log_param {
                            println!("   Compiler log (if created by compiler): {}", log_p.display());
                        }
                        // Print stdout for user to inspect if it was unrecognized
                        if !stdout_content.trim().is_empty() {
                            println!("   Compiler output (stdout):\n{}", stdout_content.trim());
                        }
                        Ok(())
                    } else {
                        log::error!(
                            "Compiler process for {:?} failed (Exit Code: {:?}) and stdout format was unrecognized.",
                            input_file, output.status.code()
                        );
                        Err(Error::GenericCompilationFailedWithLog {
                            file_path: input_file.to_path_buf(),
                            // Provide both stdout and stderr if stdout parsing failed and process failed
                            raw_log: format!(
                                "Exit Code: {:?}\nStdout:\n{}\nStderr:\n{}",
                                output.status.code(),
                                stdout_content.trim(),
                                stderr_content.trim()
                            ),
                        })
                    }
                }
            }
        }
        Err(e) => {
            // This error means `cmd.output()` itself failed (e.g., wine not found, compiler exe not found by OS)
            log::error!("Failed to execute compiler process: {}", e);
            if compiler_info.requires_wine && is_non_windows_os() && e.kind() == std::io::ErrorKind::NotFound {
                // Check if 'wine' itself was not found
                let wine_exe_check = settings.wine_path.as_deref().unwrap_or("wine");
                if e.to_string().contains(wine_exe_check) || e.to_string().contains("No such file or directory") {
                     return Err(Error::WineNotFound);
                }
            }
            Err(Error::Subprocess(e))
        }
    }
}