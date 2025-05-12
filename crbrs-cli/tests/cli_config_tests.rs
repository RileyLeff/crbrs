use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*; // Used for writing assertions
use std::process::Command; // Run programs
use tempfile::tempdir; // Create temporary directories

#[test]
fn test_config_path() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("crbrs-cli")?;
    cmd.arg("config").arg("path");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("crbrs").and(predicate::str::contains("config.toml"))); // Basic check
    Ok(())
}

#[test]
fn test_config_show_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?; // Create a temp dir for config isolation
    let config_path = temp_dir.path().join("config.toml");

    let mut cmd = Command::cargo_bin("crbrs-cli")?;
    cmd.arg("config").arg("show");
    // Override the config path for this run using an environment variable
    // (We might need to make the config loading respect an env var override,
    // or use a different approach like passing a --config flag if we add one)
    // For now, let's assume it uses the default path and ensure the file *doesn't* exist
    assert!(!config_path.exists(), "Config file should not exist for default test");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Repository URL: https://example.com/compilers.toml"))
        .stdout(predicate::str::contains("Wine Path: (Not Set - using PATH)")); // Check defaults

    temp_dir.close()?; // Clean up the temp directory
    Ok(())
}

#[test]
fn test_config_set_and_show() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let config_dir = temp_dir.path().join(".config").join("crbrs"); // Mimic structure
    let config_file = config_dir.join("config.toml");

    // --- Run 'config set' ---
    let mut cmd_set = Command::cargo_bin("crbrs-cli")?;
    cmd_set.env("XDG_CONFIG_HOME", temp_dir.path().join(".config")); // Use XDG_CONFIG_HOME for isolation
    // OR modify config.rs get_project_dirs to respect CRBRS_CONFIG_DIR env var if set
    cmd_set.arg("config").arg("set").arg("wine_path").arg("/tmp/test-wine");
    cmd_set.assert().success().stdout(predicate::str::contains("Set 'wine_path' = '/tmp/test-wine'"));

    // Verify file content (optional but good)
    assert!(config_file.exists());
    let content = std::fs::read_to_string(&config_file)?;
    assert!(content.contains("wine_path = \"/tmp/test-wine\""));


    // --- Run 'config show' ---
    let mut cmd_show = Command::cargo_bin("crbrs-cli")?;
    cmd_show.env("XDG_CONFIG_HOME", temp_dir.path().join(".config")); // Ensure same isolation
    cmd_show.arg("config").arg("show");
    cmd_show.assert()
        .success()
        .stdout(predicate::str::contains("Wine Path: /tmp/test-wine")); // Check the set value

    temp_dir.close()?;
    Ok(())
}

// Add more tests for set-association, unset-association, invalid commands etc.