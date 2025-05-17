# crbrs 🦀 - A Cross-Platform CRBasic Toolchain

Crbrs (pronounced _"cerberus"_, short for _"CRBasic in Rust"_) is a modern, cross-platform command-line interface (CLI) toolchain for developing programs for Campbell Scientific dataloggers using CRBasic.

[![crates.io](https://img.shields.io/crates/v/crbrs.svg?style=flat-square)](https://crates.io/crates/crbrs)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square)](./LICENSE-MIT)
[![Build Status](https://img.shields.io/github/actions/workflow/status/rileyleff/crbrs/rust.yml?branch=main&style=flat-square)](https://github.com/rileyleff/crbrs/actions?query=workflow%3ARust)

## Motivation

Programming Campbell Scientific dataloggers relies on proprietary Windows-based GUI tools (like the CRBasic Editor) which can feel dated and don't integrate well with modern development workflows.

`crbrs` aims to provide a better development experience by offering:

*   A **command-line interface** for common tasks like compiling, managing compilers, and (perhaps in the future) interacting with dataloggers.
*   **Cross-platform compatibility** (via wine), enabling development on macOS (including Apple Silicon/M1+ and Intel), Linux, and Windows.
*   Integration with modern text editors like **VS Code** via a basic Language Server (LSP) for diagnostics.

## Features

*   **Compiler Management:**
    *   List available compilers from a remote manifest (`compilers.toml`).
    *   Install specific compiler versions into a managed directory.
    *   List locally installed compilers.
    *   Remove installed compilers.
    *   Verify downloaded compiler archives using SHA256 checksums.
*   **Compilation Wrapper:**
    *   Compile `.cr*` files using the appropriate installed compiler.
    *   Automatically uses **Wine** on macOS/Linux to run the Windows-based Campbell Scientific compilers.
    *   Handles compiler selection based on file extension associations or explicit flags.
    *   Parses compiler output to extract structured error messages with line numbers.
*   **Configuration:**
    *   Manage settings via a `config.toml` file (e.g., compiler repository URL, Wine path, compiler storage path).
    *   Associate specific file extensions (`.cr2`, `.cr300`, etc.) with installed compiler IDs.
*   **Language Server (LSP):**
    *   Provides basic CRBasic language support for LSP-compatible editors (like VS Code).
    *   Still in development. The goal is to provide a minimal VSCode extension that compiles in the background on save, and shows some error/warning diagnostics if applicable.
*   **(Future) Datalogger Communication:**
    *   Flash compiled programs to loggers via Serial/USB.
    *   Retrieve data tables from loggers.
    *   Might involve some more complex emulation logic to route through existing campbell programs.
    *   Would love to see official Campbell Scientific support for this.

## Installation

### Prerequisites

1.  **Rust Toolchain:** Install Rust via [rustup](https://rustup.rs/).
2.  **Wine (macOS/Linux only):** Required to run the Campbell Scientific compilers. Install it through your system's package manager (e.g., `brew install wine-stable` on macOS, `sudo apt install wine` on Debian/Ubuntu). `crbrs` will try to find `wine` in your PATH, or you can specify the path via `crbrs config set wine_path /path/to/wine`.

### From Crates.io (Recommended - *Once Published*)

```bash
cargo install crbrs
```

### From Source

```bash
git clone https://github.com/rileyleff/crbrs.git
cd crbrs
cargo build --release
# The binaries will be in ./target/release/
# You can copy them to a location in your PATH, e.g., ~/.cargo/bin/
# cp target/release/crbrs target/release/crbrs-lsp ~/.cargo/bin/
```
*(Note: The CLI is named `crbrs`, and the LSP server is `crbrs-lsp`.)*

## Configuration

`crbrs` uses a configuration file (`config.toml`) stored in a standard user config location.

*   **Find Config Path:** `crbrs config path`
*   **Show Current Config:** `crbrs config show`

### Key Setting: Compiler Repository URL

`crbrs` needs to know where to find the `compilers.toml` manifest file. By default, it is configured to use the raw URL of the `compilers.toml` file on the `main` branch of the companion compiler repository: `https://raw.githubusercontent.com/RileyLeff/campbell-scientific-compilers/refs/heads/main/compilers.toml`.

While this default is convenient for development, for stability you might want to configure `crbrs` to use the `compilers.toml` file from a specific **GitHub Release asset** of the compiler repository once releases are available, or e.g. to an official Campbell Scientific repository if they ever create one.

To change the compiler repository URL, you can update the config as follows:
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

# List compilers available in the remote repository (using the configured URL)
crbrs compiler list-available

# Install a specific compiler (ID from list-available). Verifies SHA256.
crbrs compiler install cr300comp

# List compilers installed locally
crbrs compiler list

# Remove a locally installed compiler
crbrs compiler remove cr300comp

# --- Configuration ---

# Show current settings (includes default repository URL if not overridden)
crbrs config show

# Show path to config file
crbrs config path

# Set the compiler repository URL (optional override)
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

# --- Language Server (LSP) ---
# The LSP server ('crbrs-lsp') is typically started by your editor (e.g., VS Code).
# You might need to configure your editor to use the 'crbrs-lsp' executable.
# See the VS Code extension documentation (TODO: Link to VS Code extension).
```

## VS Code Integration

A basic Language Server is included (`crbrs-lsp`) that provides diagnostics by running background compilations of your code and displaying errors. A corresponding VS Code extension is planned to make setup easier.

*   **TODO:** Add a link to the VS Code extension once it's available.
*   **TODO:** Add instructions on how to manually configure VS Code to use `crbrs-lsp` in the meantime.

## Compiler Repository

The actual Campbell Scientific compiler binaries are managed in a separate repository:

➡️ [github.com/rileyleff/campbell-scientific-compilers](https://github.com/rileyleff/campbell-scientific-compilers)

This repository contains the `compilers.toml` manifest file and hosts the compiler binaries as assets on its GitHub Releases pages. Refer to that repository's README for details on its structure, automation, and crucial licensing information.

## Contributing

Contributions (bug reports, feature requests, pull requests) are welcome! Please feel free to open an issue on the [GitHub repository](https://github.com/rileyleff/crbrs/issues).

## License

Everything in the `crbrs` project (the `crbrs-lib`, `crbrs`, and `crbrs-lsp` crates, yielding the `crbrs` and `crbrs-lsp` executables) **except** for the Campbell Scientific compiler binaries themselves (which are managed in a separate repo) is licensed under either of

*   Apache License, Version 2.0, ([LICENSE-APACHE](./LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
*   MIT license ([LICENSE-MIT](./LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Disclaimer

`crbrs` is an independent project and is not affiliated with, sponsored by, or endorsed by Campbell Scientific, Inc.

The Campbell Scientific compilers managed and used by this tool are proprietary software owned by Campbell Scientific. Users are responsible for ensuring they have the appropriate licenses from Campbell Scientific to use these compilers. Refer to the [compiler repository README](https://github.com/rileyleff/campbell-scientific-compilers/blob/main/README.md) for more details.
