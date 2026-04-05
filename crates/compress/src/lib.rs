pub mod archive_extract;
pub mod codepage_437;
pub mod compress_utils;
pub mod crc;
pub mod crc_stream;
pub mod error;
/// Archive and compression abstractions.
///
/// The `compress` crate unifies handling of different archive formats (`.zip`, `.7z`, `.gz`, raw files)
/// behind a single `ICompress` trait. It allows the core engine to transparently scan, hash, and
/// (eventually) write into archives regardless of their underlying structure.
///
/// Implementation notes:
/// - Uses ecosystem crates for archive parsing and encoding.
///
/// TODO: Extend writers to preserve more archive-structure invariants (where required by validation tools).
pub mod file_header;
pub mod gzip_file;
pub mod i_compress;
pub mod native_zlib;
pub mod raw_file;
pub mod reporter;
pub mod seven_zip;
pub mod seven_zip_util;
pub mod structured_archive;
pub mod zip_enums;
pub mod zip_extra_field;
pub mod zip_file;
pub mod zstd_config;

pub use archive_extract::*;
pub use codepage_437::*;
pub use compress_utils::*;
pub use crc::*;
pub use crc_stream::*;
pub use error::*;
pub use file_header::*;
pub use gzip_file::*;
pub use i_compress::*;
pub use native_zlib::*;
pub use raw_file::*;
pub use reporter::*;
pub use seven_zip::*;
pub use seven_zip_util::*;
pub use structured_archive::*;
pub use zip_enums::*;
pub use zip_extra_field::*;
pub use zip_file::*;
pub use zstd_config::*;
