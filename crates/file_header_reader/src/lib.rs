/// Extractor for known emulator file headers.
///
/// `file_header_reader` is responsible for inspecting the first few bytes of physical files
/// to identify console-specific header wrappers (e.g. NES `.nes` headers, SNES `.smc` headers,
/// FDS headers).
///
/// Differences from C#:
/// - The logic is nearly a direct 1:1 port of the C# `FileHeaderReader` static class.
/// - It uses Rust's pattern matching to quickly return the `HeaderFileType` and offset lengths
///   needed by the scanner to calculate "headerless" CRCs.
pub mod file_headers;

pub use file_headers::*;
