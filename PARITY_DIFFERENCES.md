# RomVault Rust Port: Parity Differences

This document outlines the architectural and logic differences between the Rust port (`RustyRoms`) and the original C# `RomVault` reference implementation, discovered during documentation generation.

## 1. File & Directory Handling (`db.rs` / `DB.cs`)
- **Memory Model**: The C# version utilizes a static global class `DB` with native object references to build the file tree. The Rust port uses a `thread_local!` singleton with `Rc<RefCell<RvFile>>` and `Weak` pointers to prevent reference cycles and ensure memory safety.
- **Tree Init**: The C# version lazily initializes `ToSort` manually via the UI. The Rust version previously forced a `ToSort` creation on empty DB init, but has now been aligned to wait for the user to explicitly `Add ToSort` via the UI.

## 2. File Nodes (`rv_file.rs` / `RvFile.cs`)
- **Data Encapsulation**: C# `RvFile` logic is split across `RvFile`, `RvDir`, and `RvTreeRow` (for UI traits). The Rust implementation consolidates `RvDir` and `RvFile` logic into a single `RvFile` struct (differentiated by `FileType`), and embeds UI traits (`tree_expanded`, `tree_checked`) directly for easier `egui` rendering.
- **Cache Serialization**: C# uses custom `BinaryReader` and `BinaryWriter` for tight binary packing of the database cache. The Rust port leverages `serde` with `bincode` (and `quick-xml` for configs) for automatic and safe binary serialization.

## 3. Fixing Engine (`fix.rs` / `Fix.cs` & `FixAZipCore`)
- **Virtual IO vs Literal IO**: 
  - **C#**: Features an advanced virtual filesystem (`FixAZipCore`) that can stream data natively into and out of Zip and 7z archives without extracting to disk. It natively reformats archives into `TorrentZip` compliance on the fly.
  - **Rust**: Uses literal `fs::copy` and standard `zip` extraction. It safely moves files into place, but does not currently repackage them into new archives natively or apply `TorrentZip` structural formatting during the fix phase.
- **Operation Queue**: Both the C# and Rust versions use a state-machine task queue (`file_process_queue`) to safely process cascading file operations (moves, copies, deletes) in a single pass without causing file collisions or corrupting pointers.

## 4. Matching Logic (`find_fixes.rs` / `FindFixes.cs`)
- **Concurrency**: Both versions execute `FindFixes` using highly optimized multi-threaded workers. The Rust implementation uses `rayon::join` to concurrently build `HashMap` indexes for CRC/SHA1/MD5 to perform O(1) lookups simultaneously across CPU cores.
- **CHD and Advanced Rules**: The Rust version now implements Phase 2 match fallbacks similar to C#, properly handling SHA1/MD5-only matches, identifying older CHD versions, and falling back to size-only equivalence when cryptographic hashes are missing.

## 6. Tree Caching (`cache.rs` / `DB.Write` & `DB.Read`)
- **Binary I/O**: C# relies on a manually optimized `BinaryReader`/`BinaryWriter` loop to save the tree state bit by bit. Rust delegates entirely to the `serde` + `bincode` framework, which provides robust variable-int serialization out of the box.
- **Memory Mapping**: The Rust implementation drastically reduces cache load times by utilizing `memmap2` for zero-copy memory mapping of the `RustyRoms3_3.Cache` file directly into RAM, bypassing traditional buffered readers when possible.

## 7. Update DATs (`read_dat.rs` / `UpdateDat.cs`)
- **Parallel Parsing**: C# primarily utilizes standard BackgroundWorkers for DAT ingestion. The Rust port leverages the `rayon` crate to parallelize the actual XML/CMP text parsing across all available CPU cores before sequentially folding the resulting ASTs into the `RvFile` tree.

## 8. Physical File Scanning (`scanner.rs` / `Scanner.cs`)
- **Hashing**: C# RomVault uses advanced ThreadPool workers (`ThreadWorker`) to concurrently stream and hash files in chunks. The Rust version achieves parity by leveraging the `rayon` crate (`into_par_iter`) to parallelize file/directory discovery and hashing across all CPU cores simultaneously, utilizing 32KB buffered streams for optimal throughput.

## 9. Synchronization (`file_scanning.rs` / `FileScanning.cs` / `compare.rs`)
- **Phase 2 Matching**: C# `FileScanning` and `FileCompare` include an extensive "Phase 2" deep scan to recover CHDs, fuzzy-match size-only files, and skip erroneous file headers. The Rust port now achieves parity here, triggering an on-the-fly deep cryptographic hash check (`Scanner::scan_raw_file`) for loose files that fail the initial Phase 1 quick-match, fully mirroring the C# fallback mechanism.

## 10. Memory Efficiency (`rv_dat.rs` / `rv_game.rs`)
- **Metadata Storage**: In C# RomVault, DAT and Game metadata (like Description, RomOf, Year) are heavily packed into string arrays bounded by static enums (`DatData` / `GameData`) to save heap overhead.
- **Dynamic Vectors**: The Rust port optimizes this by using dynamically sized `Vec<DatMetaData>` vectors. Because the vast majority of `RvGame` nodes only ever populate the `Description` field, Rust entirely avoids allocating empty array slots for the other 22 unused fields, vastly reducing runtime RAM usage and disk cache size.

## 11. Fix DAT Exporting (`fix_dat_report.rs` / `FixDatReport.cs`)
- **Hierarchy Flattening**: When C# RomVault exports a "Fix DAT" (a list of missing files to download), it runs the tree through `DatClean.ArchiveDirectoryFlattern` to cleanly strip out extraneous empty virtual folders before rendering the XML. The Rust implementation now mirrors this exact behavior via `archive_directory_flatten()`, ensuring output DAT files are identical in structure to C#.

## 12. DAT Parsing (`dat_reader` crate)
- **Stream vs Full-Buffer Parsing**: The C# version utilizes stream-based `DatReader` classes that read line-by-line. The Rust port loads the entire DAT into a zero-copy (or low-copy) `Cow<str>` buffer in memory, slicing string references for the parsers (`quick-xml` for XML, custom iterators for CMP), achieving significantly higher throughput at the cost of requiring the full file size in RAM during the parse phase.

## 13. Archive Handling (`compress` crate)
- **Custom vs Ecosystem Compression**: The original C# application contains completely custom, from-scratch implementations of ZIP and 7Z byte-level manipulation, which allows it to do highly specific TorrentZip structural transformations on the fly. The Rust version utilizes the `ICompress` trait to wrap standard ecosystem crates (`zip`, `sevenz-rust`), which handles extraction perfectly but lacks the custom byte-level TorrentZip rebuilding logic during repacking.

## 14. UI Implementation (`romvault_ui`)
- **Stateful vs Immediate Mode**: The C# UI relies on `WinForms`, which means UI components like `TreeView` hold their own persistent state that is data-bound to the DB. The Rust implementation uses `egui`, an immediate-mode GUI framework. This means the entire tree is re-rendered at 60 FPS. To make this performant with hundreds of thousands of nodes, Rust heavily relies on the `cached_stats` memoization layer in `RepairStatus` to avoid recalculating branch data on every frame.
- **Dialog Management**: In C#, every popup (Settings, Mappings, About) is a dedicated `Form` class with its own `.Designer.cs` file. In the Rust port, all popup dialogs are rendered inline within a single `draw_dialogs` module, gated by boolean visibility flags on the main `RomVaultApp` struct.

## 15. CLI Tools (`dir2dat` / `trrntzip_cmd`)
- **Tool Mapping**: The Rust port exposes `Dir2Dat` and `TorrentZip` as independent binary executables (`crates/dir2dat` and `crates/trrntzip_cmd`) alongside their core library implementations. They mirror the exact command line arguments of their C# `.Net` counterparts, though their underlying execution depends on the Rust abstractions outlined above.

## 16. TorrentZip Repacking (`trrntzip`)
- **Stream Injection vs Extraction**: The C# `TrrntZip` library natively wraps `Compress.ZipFile` to execute raw byte-level repacking, allowing it to modify ZIP headers in-place without extracting files to the physical disk. The Rust version currently implements the status checking logic (`TorrentZipCheck`) but relies on a simplified rebuilding pass, as the ecosystem `zip` crate does not expose the same low-level stream injection capabilities as the custom C# library.

## 17. File Headers (`file_header_reader`)
- **1:1 Port**: The logic for identifying console-specific headers (NES, SNES, FDS) is nearly a direct 1:1 port of the C# `FileHeaderReader` static class. The Rust version utilizes pattern matching on byte slices to quickly return the `HeaderFileType` and offset lengths needed by the scanner to calculate "headerless" CRCs.

## 18. I/O Operations (`rv_io`)
- **MAX_PATH Limitations**: C# RomVault requires extensive custom `RVIO` wrappers (using P/Invoke `kernel32.dll` calls) specifically to bypass the 260-character `MAX_PATH` limitation present in older Windows `.NET` frameworks. Rust's standard library (`std::fs`) inherently supports long paths (`\\?\`) on modern Windows out of the box, so `rv_io` mostly acts as a thin semantic mapping layer (providing classes like `DirectoryInfo`, `FileInfo`) for easier code porting, rather than a mandatory low-level system bypass.

## 19. External DAT Conversion (`external_dat_converter_to.rs`)
- **Hierarchy Flattening**: Similar to the Fix DAT exporter, when the C# UI exports an existing branch of the tree as a standalone DAT file, it runs through complex flattening rules (`DatClean.ArchiveDirectoryFlattern`) to strip out unneeded structural nodes. The Rust port executes a strict 1:1 mapping, outputting the literal internal tree state into the `DatHeader` AST.

## 20. Headless Operation (`rom_vault`)
- **CLI vs GUI**: The original C# application (`RomVault`) only shipped as a tightly coupled WinForms desktop application. The Rust port decouples the engine (`rv_core`) entirely from the GUI (`romvault_ui`) and introduces a dedicated `rom_vault` CLI binary. This allows the exact same Rust engine to be run interactively on headless Linux servers, Docker containers, or NAS systems for automated ROM management.

## 21. Alternative UI Architectures (`trrntzip_ui` / `rustyvault_tauri`)
- **Future Expansion**: The workspace contains scaffolding for a `rustyvault_tauri` frontend. Because the core engine (`rv_core`) is decoupled from the `egui` frontend, the application can theoretically be compiled into a lightweight web-view UI using Tauri, a feature impossible in the tightly-coupled C# `WinForms` architecture. Similarly, `trrntzip_ui` exists as a stub to eventually port the standalone `TorrentZip.Net` GUI tool.

## 22. XML Exporting (`xml_writer.rs`)
- **Format Specialization**: The C# `DatClean` logic and `FixDat` writers contain highly specialized XML string writers with deep formatting rules to perfectly emulate different DAT engine DOCTYPE headers (e.g., MAME vs ClrMamePro vs Logiqx). The Rust `xml_writer.rs` now fully implements this emulation, including dynamic root `<machine>` vs `<game>` switching, custom DTD injections, and exact XML entity escaping rules.

## 23. Solid Archive Handling (`seven_zip.rs`)
- **Stream Optimization**: The C# `Compress.SevenZip` library is a massively complex custom LZMA decoder built specifically to handle solid-block streaming and chunked hashing without extracting the entire solid block to disk. The Rust version utilizes the `sevenz-rust` ecosystem crate. It successfully reads and extracts files, but currently lacks the granular solid-block stream-hashing memory optimizations present in the custom C# engine, meaning it may use more memory when scanning very large solid 7z archives.

## 24. Zip Modification Pipeline (`torrent_zip_rebuild.rs`)
- **In-Place Mutation**: The C# `TorrentZipRebuild` relies on a custom `Compress.ZipFile` writer that can modify streams and rename internal files in-place while adhering to the TorrentZip specification. The Rust implementation currently acts as a structural verifier (`TorrentZipCheck`) but stubs out the `TorrentZipRebuild` byte-writing pass, pending integration with a low-level zip stream encoder.