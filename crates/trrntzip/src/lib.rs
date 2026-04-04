pub mod process_control;
pub mod process_zip;
pub mod torrent_zip;
pub mod torrent_zip_check;
pub mod torrent_zip_make;
pub mod torrent_zip_rebuild;
/// Core logic for verifying and rebuilding TorrentZip archives.
///
/// `trrntzip` provides the programmatic API for inspecting standard `.zip` files
/// and converting them into deterministic `TorrentZip` format (where file ordering,
/// timestamps, and compression methods are strictly standardized).
///
/// Differences from C#:
/// - The C# `TrrntZip` library natively wraps `Compress.ZipFile` to execute raw byte-level
///   repacking without extracting files to the physical disk.
/// - The Rust version currently implements the status checking logic (`TorrentZipCheck`)
///   but relies on a simplified or incomplete rebuilding pass (`TorrentZipRebuild`),
///   as the `zip` crate does not expose the same low-level stream injection capabilities
///   as the custom C# library.
pub mod trrntzip_status;
pub mod zipped_file;

pub use process_control::*;
pub use process_zip::*;
pub use torrent_zip::*;
pub use torrent_zip_check::*;
pub use torrent_zip_make::*;
pub use torrent_zip_rebuild::*;
pub use trrntzip_status::*;
pub use zipped_file::*;
