// FILE: crbrs-lib/src/installer.rs

use crate::{Error, Manifest, CompilerInfo, Settings}; // ManifestCompilerEntry is not directly used here now
use crate::config::{get_compiler_storage_path, save_settings};
use reqwest::blocking::Client;
use std::fs::{self, File}; // File might not be strictly needed if not writing intermediate files
use std::io::{self, Cursor}; // Removed Read, Write if not directly used
use std::path::{Path, PathBuf};
use zip::ZipArchive;

// --- Add imports for SHA256 ---
use sha2::{Digest, Sha256}; // <-- NEW IMPORTS

/// Fetches the compiler manifest from the given URL.
pub fn fetch_manifest(repository_url: &str) -> Result<Manifest, Error> {
    // ... (implementation remains the same) ...
    log::info!("Fetching compiler manifest from: {}", repository_url);
    let client = Client::builder().build()?;
    let response = client.get(repository_url).send()?;
    if !response.status().is_success() {
        log::error!("Failed to fetch manifest. Status: {:?}, URL: {}", response.status(), repository_url);
        return Err(Error::Network(response.error_for_status().unwrap_err()));
    }
    let manifest_text = response.text()?;
    let manifest: Manifest = toml::from_str(&manifest_text)
        .map_err(|e| Error::InvalidCompilerSource(format!("Failed to parse manifest TOML: {}", e)))?;
    log::info!("Successfully fetched and parsed manifest. {} compilers listed.", manifest.compilers.len());
    Ok(manifest)
}


/// Installs a compiler specified by its ID from the manifest.
/// Modifies the `settings` in place and saves them.
pub fn install_compiler(
    settings: &mut Settings,
    compiler_id_to_install: &str,
) -> Result<(), Error> {
    let manifest = fetch_manifest(&settings.compiler_repository_url)?;

    let entry = manifest // This is ManifestCompilerEntry
        .compilers
        .get(compiler_id_to_install)
        .ok_or_else(|| Error::CompilerIdNotFoundInManifest(compiler_id_to_install.to_string()))?;

    log::info!("Attempting to install compiler: '{}' (Version: {}) from {}",
        compiler_id_to_install, entry.version, entry.download_url);

    // 1. Download the compiler archive
    let client = Client::new();
    let response = client.get(&entry.download_url).send()?;
    if !response.status().is_success() {
        return Err(Error::Network(response.error_for_status().unwrap_err()));
    }
    let archive_bytes = response.bytes()?.to_vec();
    log::info!("Downloaded {} bytes for compiler '{}'", archive_bytes.len(), compiler_id_to_install);

    // --- 2. Verify SHA256 checksum ---
    if let Some(expected_sha256_from_manifest) = &entry.sha256 {
        if !expected_sha256_from_manifest.is_empty() { // Only verify if a hash is provided
            log::info!("Verifying SHA256 checksum for '{}'...", compiler_id_to_install);
            let mut hasher = Sha256::new();
            hasher.update(&archive_bytes);
            let actual_sha256_bytes = hasher.finalize();
            // Convert bytes to hex string.
            // Using a simple loop; crates like `hex` or `data-encoding` could also be used.
            let actual_sha256_hex = actual_sha256_bytes
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<String>();

            if actual_sha256_hex.to_lowercase() != expected_sha256_from_manifest.to_lowercase() {
                log::error!(
                    "SHA256 checksum mismatch for {}. Expected: '{}', Got: '{}'",
                    compiler_id_to_install,
                    expected_sha256_from_manifest,
                    actual_sha256_hex
                );
                return Err(Error::ChecksumMismatch {
                    compiler_id: compiler_id_to_install.to_string(),
                    expected: expected_sha256_from_manifest.clone(),
                    actual: actual_sha256_hex,
                });
            }
            log::info!("SHA256 checksum verified successfully for '{}'", compiler_id_to_install);
        } else {
            log::warn!("No SHA256 checksum provided in manifest for '{}'. Skipping verification.", compiler_id_to_install);
        }
    } else {
        log::warn!("No SHA256 checksum provided in manifest for '{}'. Skipping verification.", compiler_id_to_install);
    }
    // --- End SHA256 Verification ---

    // 3. Determine storage path and unpack
    let compiler_base_storage_path = get_compiler_storage_path(settings)?;
    let install_subdir = PathBuf::from(compiler_id_to_install);
    let compiler_install_path = compiler_base_storage_path.join(&install_subdir);

    if compiler_install_path.exists() {
        log::warn!("Compiler installation path {:?} already exists. Removing existing version first.", compiler_install_path);
        fs::remove_dir_all(&compiler_install_path)?;
    }
    fs::create_dir_all(&compiler_install_path)?;
    log::info!("Created installation directory: {:?}", compiler_install_path);

    let reader = Cursor::new(archive_bytes); // Use the downloaded bytes
    let mut archive = ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        // Sanitize file path to prevent zip slip vulnerabilities
        let outpath = match file.enclosed_name() {
            Some(path) => compiler_install_path.join(path),
            None => {
                log::warn!("Skipping potentially unsafe file path in zip: {}", file.name());
                continue;
            }
        };

        if file.name().ends_with('/') {
            log::debug!("Creating directory from zip: {:?}", outpath);
            fs::create_dir_all(&outpath)?;
        } else {
            log::debug!("Extracting file from zip: {:?} ({} bytes)", outpath, file.size());
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                if mode != 0 { // Only set permissions if mode is non-zero
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
                }
            }
        }
    }
    log::info!("Successfully unpacked compiler '{}' to {:?}", compiler_id_to_install, compiler_install_path);

    // 4. Update settings
    let installed_info = CompilerInfo {
        id: compiler_id_to_install.to_string(),
        description: entry.description.clone(),
        version: entry.version.clone(),
        install_subdir,
        executable_name: entry.executable_name.clone(),
        requires_wine: entry.requires_wine,
        supported_loggers: entry.supported_loggers.clone(),
    };
    settings.installed_compilers.insert(compiler_id_to_install.to_string(), installed_info);

    // 5. Save settings
    save_settings(settings)?;
    log::info!("Compiler '{}' installed and settings saved.", compiler_id_to_install);

    Ok(())
}

/// Removes an installed compiler.
pub fn remove_compiler(settings: &mut Settings, compiler_id_to_remove: &str) -> Result<(), Error> {
    if !settings.installed_compilers.contains_key(compiler_id_to_remove) {
        log::warn!("Compiler '{}' not found in settings, nothing to remove.", compiler_id_to_remove);
        return Ok(());
    }
    let compiler_base_storage_path = get_compiler_storage_path(settings)?;
    let compiler_install_dir = compiler_base_storage_path.join(compiler_id_to_remove);
    if compiler_install_dir.exists() {
        log::info!("Removing compiler directory: {:?}", compiler_install_dir);
        fs::remove_dir_all(&compiler_install_dir)?;
    } else {
        log::warn!("Compiler directory {:?} not found, but removing from settings anyway.", compiler_install_dir);
    }
    settings.installed_compilers.remove(compiler_id_to_remove);
    save_settings(settings)?;
    log::info!("Compiler '{}' removed and settings saved.", compiler_id_to_remove);
    Ok(())
}