# RustyRoms (RustyVault)

**RustyRoms** is a high-performance, cross-platform Rust port of the popular **RomVault** ROM management utility. 

It provides an extremely fast engine for scanning, verifying, and organizing massive collections of emulation files using industry-standard DAT files (Logiqx XML, ClrMamePro, RomCenter). By leveraging Rust's memory safety and zero-copy abstractions, RustyRoms aims to significantly accelerate the ROM auditing pipeline while remaining structurally compatible with the original C# RomVault implementation.

## Features

- **Extreme Performance**: DAT files are parsed in parallel using zero-copy DOM abstractions (`roxmltree`). Physical file scanning is deeply optimized.
- **Cross-Platform**: Unlike the original Windows-only `WinForms` application, RustyRoms runs natively on Windows, Linux, and macOS.
- **Decoupled Architecture**: The core engine (`rv_core`) is completely decoupled from the UI, allowing it to be run as a standard desktop application or a headless CLI server.
- **Immediate Mode GUI**: The desktop interface is built on `egui`, capable of rendering hundreds of thousands of tree nodes at 60 FPS.
- **TorrentZip Support**: Built-in verification for the deterministic `TorrentZip` archive format.

## Project Structure

The project is organized as a Cargo workspace containing multiple specialized crates:

### Core Engine
* `crates/rv_core`: The primary ROM management engine. Handles the internal database (`RvFile` tree), physical disk scanning, fixing logic, and binary cache serialization.
* `crates/dat_reader`: High-speed parsers for various DAT formats (XML, CMP, RomCenter, MESS).
* `crates/compress`: Unified `ICompress` trait and wrapper implementations for interacting with raw files, `.zip`, `.7z`, and `.gz` archives.
* `crates/file_header_reader`: Emulator-specific file header detection (NES, SNES, FDS, LYNX, etc.).
* `crates/rv_io`: Cross-platform file I/O abstractions masking differences between standard Rust `std::fs` and legacy Windows long-path behaviors.

### Frontends & Tools
* `crates/romvault_ui`: The primary graphical user interface built with `egui`.
* `crates/rom_vault`: A headless command-line interface (CLI) wrapper for the core engine.
* `crates/trrntzip`: The core engine for verifying and rebuilding `TorrentZip` archives.
* `crates/trrntzip_cmd`: A CLI tool for executing TorrentZip operations.
* `crates/dir2dat`: A CLI tool that converts an existing physical directory into a Logiqx XML DAT file.

## Getting Started

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (Cargo)
- Standard build tools for your OS (e.g., MSVC on Windows, `build-essential` on Linux)

### Building & Running

To run the primary Graphical User Interface:
```bash
cargo run -p romvault_ui --release
```

To run the headless CLI engine:
```bash
cargo run -p rom_vault --release
```

To run the standalone TorrentZip CLI tool:
```bash
cargo run -p trrntzip_cmd --release -- <path_to_zip>
```

### Documentation

The entire codebase is extensively documented using `rustdoc`. Every module and architectural decision is cross-referenced with its C# RomVault counterpart.

To generate and view the local documentation:
```bash
cargo doc --no-deps --document-private-items --open
```

You can also view the complete architectural comparison between this Rust port and the original C# application in [PARITY_DIFFERENCES.md](PARITY_DIFFERENCES.md).

## Development Status

RustyRoms is actively under development. Currently, it acts as an extremely fast DAT parser and file scanner. Full parity with the C# `TorrentZipRebuild` in-place byte modification logic is currently being implemented.

## License

Currently no licensing.
