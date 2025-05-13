# crbrs 🦀 - A Cross-Platform CRBasic Toolchain

A modern, cross-platform command-line interface (CLI) toolchain for developing programs for Campbell Scientific dataloggers using CRBasic, written in Rust.

[![crates.io](https://img.shields.io/crates/v/crbrs-cli.svg?style=flat-square)](https://crates.io/crates/crbrs-cli) <!-- Placeholder -->
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square)](./LICENSE-MIT) <!-- Choose MIT/Apache or just one -->
[![Build Status](https://img.shields.io/github/actions/workflow/status/rileyleff/crbrs/rust.yml?branch=main&style=flat-square)](https://github.com/rileyleff/crbrs/actions?query=workflow%3ARust) <!-- Placeholder for your CI workflow file -->

## Motivation

Programming Campbell Scientific dataloggers often involves using proprietary Windows-based GUI tools (like the CRBasic Editor) which can feel dated and don't integrate well with modern development workflows.

`crbrs` aims to provide a better development experience by offering:

*   A **command-line interface** for common tasks like compiling, managing compilers, and (eventually) interacting with dataloggers.
*   **Cross-platform compatibility**, enabling development on macOS (including Apple Silicon/M1+ and Intel), Linux, and Windows.
*   Integration with modern text editors like **VS Code** (initially via external tasks, planned LSP support).
*   A way to manage different **compiler versions** needed for various datalogger families and OS versions.

## Features

*   **Compiler Management:**
    *   List available compilers from a remote manifest (`compilers.toml`).
    *   Install specific compiler versions into a managed directory.
    *   List locally installed compilers.
    *   Remove installed compilers.
*   **Compilation Wrapper:**
    *   Compile `.cr*` files using the appropriate installed compiler.
    *   Automatically uses **Wine** on macOS/Linux to run the Windows-based Campbell Scientific compilers.
    *   Handles compiler selection based on file extension associations or explicit flags.
*   **Configuration:**
    *   Manage settings via a `config.toml` file (e.g., compiler repository URL, Wine path).
    *   Associate specific file extensions (`.cr2`, `.cr300`, etc.) with installed compiler IDs.
*   **(Planned) Datalogger Communication:**
    *   Flash compiled programs to loggers via Serial/USB.
    *   Retrieve data tables from loggers.
*   **(Planned) Language Server (LSP):**
    *   Provide basic CRBasic language support (diagnostics via background compilation) for LSP-compatible editors like VS Code.

## Installation

### Prerequisites

1.  **Rust Toolchain:** Install Rust via [rustup](https://rustup.rs/).
2.  **Wine (macOS/Linux only):** Required to run the Campbell Scientific compilers. Install it through your system's package manager (e.g., `brew install wine-stable` on macOS, `sudo apt install wine` on Debian/Ubuntu). `crbrs` will try to find `wine` in your PATH, or you can specify the path via `crbrs config set wine_path /path/to/wine`.

### From Crates.io (Recommended - *Once Published*)

```bash
cargo install crbrs-cli
```

### From Source

```bash
git clone https://github.com/rileyleff/crbrs.git
cd crbrs
cargo build --release
# The binary will be in ./target/release/crbrs-cli
# You can copy it to a location in your PATH, e.g., ~/.cargo/bin/
# cp target/release/crbrs-cli ~/.cargo/bin/crbrs
```

## Configuration

`crbrs` uses a configuration file (`config.toml`) stored in a standard user config location.

*   **Find Config Path:** `crbrs config path`
*   **Show Current Config:** `crbrs config show`

### Key Setting: Compiler Repository URL

`crbrs` needs to know where to find the `compilers.toml` manifest file. It defaults to an example URL. You should point it to the manifest provided by the companion repository:

1.  **Get the Raw URL:** Go to [github.com/rileyleff/campbell-scientific-compilers](https://github.com/rileyleff/campbell-scientific-compilers), navigate to `compilers.toml`, click "Raw", and copy the URL.
2.  **Set the URL:**
    ```bash
    crbrs config set compiler_repository_url <PASTE_RAW_URL_HERE>
    ```

### Other Settings

*   `wine_path`: (Optional) Explicit path to the `wine` executable if not in your system PATH.
*   `compiler_storage_path`: (Optional) Override the default location where compiler zips are unpacked.
*   `file_associations`: Map file extensions to compiler IDs (see Usage).

## Usage

```bash
# General Help
crbrs --help
crbrs compile --help
crbrs compiler --help
crbrs config --help

# --- Compiler Management ---

# List compilers available in the remote repository
crbrs compiler list-available

# Install a specific compiler (ID from list-available)
crbrs compiler install cr300comp

# List compilers installed locally
crbrs compiler list

# Remove a locally installed compiler
crbrs compiler remove cr300comp

# --- Configuration ---

# Show current settings
crbrs config show

# Show path to config file
crbrs config path

# Set the compiler repository URL
crbrs config set compiler_repository_url <URL>

# Set the path to Wine (if needed)
crbrs config set wine_path /opt/local/bin/wine

# Associate .cr2 files with the 'cr2comp-v10s' compiler
crbrs config set-association --extension cr2 --compiler-id cr2comp-v10s

# Remove an association
crbrs config unset-association --extension cr2

# --- Compilation ---

# Compile using the associated compiler for .cr2
crbrs compile my_program.cr2

# Compile and specify an output log file
crbrs compile my_program.cr2 --output-log compile_log.txt

# Compile using a specific compiler, overriding association
crbrs compile my_other_program.cr2 --compiler cr2comp-cr200x-std-04
```

## Compiler Repository

The actual Campbell Scientific compiler binaries are managed in a separate repository:

➡️ [github.com/rileyleff/campbell-scientific-compilers](https://github.com/rileyleff/campbell-scientific-compilers)

This repository contains:

*   The `compilers.toml` manifest file that `crbrs` reads.
*   Scripts to package compilers.
*   Automation (GitHub Actions) to create GitHub Releases.
*   The **compiler binaries themselves hosted as assets** on the GitHub Releases page of that repository.

Please refer to that repository's README for details on compiler licensing and management.

## Future Work / Roadmap

*   [ ] Implement actual compilation execution logic (calling the compiler via Wine/natively).
*   [ ] Implement datalogger communication (flashing, data retrieval).
*   [ ] Develop a basic Language Server Protocol (LSP) implementation.
*   [ ] Add more robust error handling and reporting.
*   [ ] Publish `crbrs-cli` to crates.io.
*   [ ] Provide pre-built binaries via GitHub Releases for `crbrs`.

## Contributing

Contributions (bug reports, feature requests, pull requests) are welcome! Please feel free to open an issue on the [GitHub repository](https://github.com/rileyleff/crbrs/issues).

## License

Everything in the project (`crbrs`) **except** for the compiler binaries themselves (which are hosted in a separate repo) is licensed under either of

*   Apache License, Version 2.0, ([LICENSE-APACHE](./LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
*   MIT license ([LICENSE-MIT](./LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Disclaimer

`crbrs` is an independent project and is not affiliated with, sponsored by, or endorsed by Campbell Scientific, Inc.

The Campbell Scientific compilers managed and used by this tool are proprietary software owned by Campbell Scientific. Users are responsible for ensuring they have the appropriate licenses from Campbell Scientific to use these compilers. Refer to the [compiler repository README](https://github.com/rileyleff/campbell-scientific-compilers/blob/main/README.md) for more details.
