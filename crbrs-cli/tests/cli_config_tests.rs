// FILE: crbrs-cli/tests/cli_config_tests.rs

use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::path::{Path, PathBuf};
use std::process::Command; // Run programs
use tempfile::TempDir; // Create temporary directories automatically cleaned up

// --- Configuration ---
// !!! IMPORTANT: This MUST match the AppName used in ProjectDirs::from() in crbrs-lib/src/config.rs !!!
// Example: If using ProjectDirs::from("cli.crbrs", "", "crbrs")
const APP_NAME: &str = "crbrs";
const CONFIG_FILENAME: &str = "config.toml";

// --- Helper Functions for Test Isolation ---

/// Creates a Command for `crbrs-cli` configured to use isolated config/data directories.
/// Sets XDG_CONFIG_HOME and XDG_DATA_HOME environment variables pointing inside the temp_dir.
fn crbrs_cmd_isolated(temp_dir: &TempDir) -> Result<Command, Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("crbrs-cli")?;
    // Redirect config and data directories using standard XDG environment variables
    cmd.env("XDG_CONFIG_HOME", temp_dir.path().join("config"));
    cmd.env("XDG_DATA_HOME", temp_dir.path().join("data"));
    // Optional: Clear other potentially interfering env vars if needed
    // cmd.env_remove("HOME"); // Be careful with this, might break things unexpectedly
    Ok(cmd)
}

/// Calculates the expected path to the config file within the isolated environment.
fn get_isolated_config_file_path(temp_dir: &TempDir) -> PathBuf {
    temp_dir
        .path()
        .join("config") // Matches XDG_CONFIG_HOME subdir
        .join(APP_NAME) // Matches the AppName used by ProjectDirs
        .join(CONFIG_FILENAME)
}

// --- Test Cases ---

#[test]
fn test_config_path_default() -> Result<(), Box<dyn std::error::Error>> {
    // Test the *default* path resolution without isolation override
    // This will vary based on the OS (e.g., ~/Library/... on macOS)
    let mut cmd = Command::cargo_bin("crbrs-cli")?;
    cmd.arg("config").arg("path");
    cmd.assert()
        .success()
        // Check that it contains standard components, adjust based on your ProjectDirs settings
        .stdout(
            predicate::str::contains(APP_NAME)
                .and(predicate::str::contains(CONFIG_FILENAME))
                // Add more specific checks if needed, e.g., ".config" on Linux, "Library" on macOS
                .and(predicate::str::contains("Library/Application Support").or(predicate::str::contains(".config"))) // Example OS check
        );
    Ok(())
}

#[test]
fn test_config_show_defaults_isolated() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?; // Creates temp dir for this test
    let mut cmd = crbrs_cmd_isolated(&temp_dir)?; // Command uses isolated paths
    cmd.arg("config").arg("show");

    // Calculate the path the command *should* look for config in
    let isolated_config = get_isolated_config_file_path(&temp_dir);

    // Verify config file doesn't exist *before* running 'show' in isolated env
    assert!(
        !isolated_config.exists(),
        "Config file {:?} should not exist in isolated temp dir before running 'show'",
        isolated_config
    );

    // Run 'config show' and check for default output
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Repository URL: https://example.com/compilers.toml", // Check a default value
        ))
        .stdout(predicate::str::contains(
            "Wine Path: (Not Set - using PATH)", // Check default wine path message
        ));

    // Verify config file *still* doesn't exist after *showing* defaults
    assert!(
        !isolated_config.exists(),
        "Config file {:?} should still not exist after merely showing defaults",
        isolated_config
    );

    Ok(()) // temp_dir is automatically cleaned up when it goes out of scope
}

#[test]
fn test_config_set_and_show_isolated() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let isolated_config = get_isolated_config_file_path(&temp_dir);
    let test_wine_path = "/test/path/to/wine"; // Use a unique path for testing

    // --- 1. Run 'config set' in isolated environment ---
    let mut cmd_set = crbrs_cmd_isolated(&temp_dir)?;
    cmd_set
        .arg("config")
        .arg("set")
        .arg("wine_path")
        .arg(test_wine_path);

    cmd_set.assert().success().stdout(
        predicate::str::contains("Set 'wine_path'")
            .and(predicate::str::contains(test_wine_path)),
    );

    // --- 2. Verify the config file was created and has the correct content ---
    assert!(
        isolated_config.exists(),
        "Config file {:?} should have been created by 'config set'",
        isolated_config
    );
    let content = std::fs::read_to_string(&isolated_config)?;
    println!("DEBUG: Isolated config content:\n{}", content); // Print for debugging if needed
    assert!(
        content.contains(&format!("wine_path = \"{}\"", test_wine_path)),
        "Config file content mismatch"
    );

    // --- 3. Run 'config show' in the SAME isolated environment ---
    let mut cmd_show = crbrs_cmd_isolated(&temp_dir)?;
    cmd_show.arg("config").arg("show");

    // Check that 'config show' now reflects the value we set
    cmd_show
        .assert()
        .success()
        .stdout(predicate::str::contains(&format!(
            "Wine Path: {}",
            test_wine_path
        )));

    Ok(())
}

#[test]
fn test_config_set_association_isolated() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    let isolated_config = get_isolated_config_file_path(&temp_dir);

    // --- 1. Set Association ---
    let mut cmd_set = crbrs_cmd_isolated(&temp_dir)?;
    cmd_set
        .arg("config")
        .arg("set-association")
        .arg("--extension")
        .arg("crTest") // Use a test-specific extension
        .arg("--compiler-id")
        .arg("test-compiler-v1");
    cmd_set.assert().success();

    // --- 2. Verify File Content ---
    assert!(isolated_config.exists());
    let content = std::fs::read_to_string(&isolated_config)?;
    assert!(content.contains("[file_associations]"));
    assert!(content.contains("crtest = \"test-compiler-v1\"")); // Check lowercase key

    // --- 3. Show and Verify ---
    let mut cmd_show = crbrs_cmd_isolated(&temp_dir)?;
    cmd_show.arg("config").arg("show");
    cmd_show
        .assert()
        .success()
        .stdout(predicate::str::contains(".crtest -> test-compiler-v1"));

    // --- 4. Unset Association ---
    let mut cmd_unset = crbrs_cmd_isolated(&temp_dir)?;
     cmd_unset
        .arg("config")
        .arg("unset-association")
        .arg("--extension")
        .arg("crTest"); // Should handle case-insensitivity / cleaning
     cmd_unset.assert().success();

     // --- 5. Verify File Content After Unset ---
     let content_after_unset = std::fs::read_to_string(&isolated_config)?;
     assert!(!content_after_unset.contains("crtest =")); // Association should be gone

     // --- 6. Show After Unset ---
      let mut cmd_show_after = crbrs_cmd_isolated(&temp_dir)?;
      cmd_show_after.arg("config").arg("show");
      cmd_show_after
         .assert()
         .success()
         .stdout(predicate::str::contains("File Associations:\n    (None)")); // Assumes no others were set

    Ok(())
}

#[test]
fn test_config_set_invalid_key() -> Result<(), Box<dyn std::error::Error>> {
     let temp_dir = TempDir::new()?;
     let mut cmd = crbrs_cmd_isolated(&temp_dir)?;
     cmd.arg("config").arg("set").arg("this_key_is_bad").arg("some_value");
     cmd.assert()
        .failure() // Expect non-zero exit code
        .stderr(predicate::str::contains("Error: Configuration error: Unknown configuration key: this_key_is_bad"));
    Ok(())
}

// Add more tests as needed for edge cases, other commands (compiler list initially), etc.