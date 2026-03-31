/// Archive and compression abstractions.
/// 
/// The `compress` crate unifies handling of different archive formats (`.zip`, `.7z`, `.gz`, raw files)
/// behind a single `ICompress` trait. It allows the core engine to transparently scan, hash, and 
/// (eventually) write into archives regardless of their underlying structure.
/// 
/// Differences from C#:
/// - The C# `Compress` library contains completely custom, from-scratch implementations of ZIP and 7Z 
///   parsing optimized specifically for ROM management and TorrentZip structure preservation.
/// - The Rust version acts primarily as an abstraction layer over robust ecosystem crates (`zip`, `sevenz-rust`),
///   though it retains the same `ICompress` API surface to ensure compatibility with the rest of the port.
pub mod file_header;
pub mod structured_archive;
pub mod zip_enums;
pub mod i_compress;
pub mod zip_file;
pub mod seven_zip;
pub mod raw_file;
pub mod gzip_file;

pub use file_header::*;
pub use structured_archive::*;
pub use zip_enums::*;
pub use i_compress::*;
pub use zip_file::*;
pub use seven_zip::*;
pub use raw_file::*;
pub use gzip_file::*;
