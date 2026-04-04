pub mod directory;
pub mod directory_info;
/// Cross-platform file I/O abstractions.
///
/// `rv_io` provides a wrapper layer over standard Rust `std::fs` and `std::path` APIs to
/// simulate the behavior of the C# `System.IO` and custom RomVault IO wrapper classes.
/// This includes long-path support (`\\?\`) for Windows, which is natively handled by
/// Rust's `std::fs` on modern Windows versions but is abstracted here for semantic parity
/// with the C# source.
///
/// Differences from C#:
/// - C# RomVault uses extensive custom `RVIO` wrappers specifically to bypass the 260-character
///   `MAX_PATH` limitation in older Windows `.NET` frameworks using P/Invoke `kernel32.dll` calls.
/// - Rust's standard library inherently supports long paths on Windows, so `rv_io` mostly acts
///   as a thin semantic mapping layer (e.g., `DirectoryInfo`, `FileInfo`) rather than a
///   mandatory low-level bypass.
pub mod file;
pub mod file_info;
pub mod file_stream;
pub mod name_fix;
pub mod path;

pub use directory::*;
pub use directory_info::*;
pub use file::*;
pub use file_info::*;
pub use file_stream::*;
pub use name_fix::*;
pub use path::*;
