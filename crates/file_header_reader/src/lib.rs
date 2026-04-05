/// Extractor for known emulator file headers.
///
/// `file_header_reader` is responsible for inspecting the first few bytes of physical files
/// to identify console-specific header wrappers (e.g. NES `.nes` headers, SNES `.smc` headers,
/// FDS headers).
///
/// Implementation notes:
/// - Uses pattern matching on magic bytes to determine the header type and offset.
pub mod file_headers;

pub use file_headers::*;
