// FILE: crbrs-lib/src/installer.rs

use crate::{Error, Manifest, ManifestCompilerEntry, CompilerInfo, Settings};
use crate::config::{get_compiler_storage_path, save_settings}; // Assuming save_settings is pub
use reqwest::blocking::Client; // Using blocking client for simplicity for now
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write}; // Added Write for file saving
use std::path::{Path, PathBuf};
use zip::ZipArchive; // Make sure `zip` crate is in crbrs-lib/Cargo.toml

// If you plan to do SHA256 verification
// use sha2::{Sha256, Digest};


/// Fetches the compiler manifest from the given URL.
pub fn fetch_manifest(repository_url: &str) -> Result<Manifest, Error> {
    log::info!("Fetching compiler manifest from: {}", repository_url);
    let client = Client::builder().build()?; // Or new_pkcs8_client for TLS if needed
    let response = client.get(repository_url).send()?;

    if !response.status().is_success() {
        log::error!(
            "Failed to fetch manifest. Status: {:?}, URL: {}",
            response.status(),
            repository_url
        );
        return Err(Error::Network(response.error_for_status().unwrap_err())); // Convert to reqwest::Error
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

    let entry = manifest
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
    let archive_bytes = response.bytes()?.to_vec(); // Read all bytes into memory
    log::info!("Downloaded {} bytes for compiler '{}'", archive_bytes.len(), compiler_id_to_install);

    // 2. (Optional) Verify SHA256 checksum
    if let Some(expected_sha256) = &entry.sha256 {
        // Placeholder for SHA256 verification
        // let mut hasher = Sha256::new();
        // hasher.update(&archive_bytes);
        // let actual_sha256 = format!("{:x}", hasher.finalize());
        // if &actual_sha256 != expected_sha256 {
        //     return Err(Error::InvalidCompilerSource(format!(
        //         "SHA256 checksum mismatch for {}. Expected: {}, Got: {}",
        //         compiler_id_to_install, expected_sha256, actual_sha256
        //     )));
        // }
        // log::info!("SHA256 checksum verified for '{}'", compiler_id_to_install);
        log::warn!("SHA256 verification for '{}' is currently a placeholder.", compiler_id_to_install);
    }


    // 3. Determine storage path and unpack
    let compiler_base_storage_path = get_compiler_storage_path(settings)?;
    let install_subdir = PathBuf::from(compiler_id_to_install); // Use compiler ID as subdir name
    let compiler_install_path = compiler_base_storage_path.join(&install_subdir);

    if compiler_install_path.exists() {
        log::warn!("Compiler installation path {:?} already exists. Overwriting.", compiler_install_path);
        fs::remove_dir_all(&compiler_install_path)?; // Remove if exists to ensure clean install
    }
    fs::create_dir_all(&compiler_install_path)?;
    log::info!("Created installation directory: {:?}", compiler_install_path);

    // Unpack ZIP archive
    let reader = Cursor::new(archive_bytes);
    let mut archive = ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = compiler_install_path.join(file.mangled_name());

        if file.name().ends_with('/') {
            log::debug!("Creating directory within zip: {:?}", outpath);
            fs::create_dir_all(&outpath)?;
        } else {
            log::debug!("Extracting file within zip: {:?} ({} bytes)", outpath, file.size());
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(&p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
        // Set permissions if on Unix (optional, might be needed for executables)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
            }
        }
    }
    log::info!("Successfully unpacked compiler '{}' to {:?}", compiler_id_to_install, compiler_install_path);


    // 4. Update settings
    let installed_info = CompilerInfo {
        id: compiler_id_to_install.to_string(),
        description: entry.description.clone(),
        version: entry.version.clone(),
        install_subdir, // Relative path to the specific compiler's directory
        executable_name: entry.executable_name.clone(),
        requires_wine: entry.requires_wine,
        supported_loggers: entry.supported_loggers.clone(),
    };
    settings.installed_compilers.insert(compiler_id_to_install.to_string(), installed_info);

    // 5. Save settings
    save_settings(settings)?; // save_settings needs to be pub in config.rs
    log::info!("Compiler '{}' installed and settings saved.", compiler_id_to_install);

    Ok(())
}

/// Removes an installed compiler.
/// Modifies `settings` in place and saves them.
pub fn remove_compiler(settings: &mut Settings, compiler_id_to_remove: &str) -> Result<(), Error> {
    if !settings.installed_compilers.contains_key(compiler_id_to_remove) {
        log::warn!("Compiler '{}' not found in settings, nothing to remove.", compiler_id_to_remove);
        return Ok(()); // Or return an error like CompilerNotFound
    }

    let compiler_base_storage_path = get_compiler_storage_path(settings)?;
    // Assuming the subdir name is the compiler ID itself, as used in install_compiler
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