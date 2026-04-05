//! ZIP archive support.
//!
//! This module provides [`ZipFile`], an [`ICompress`](crate::i_compress::ICompress) implementation
//! for `.zip` archives.
//!
//! `ZipFile` supports:
//! - Reading file entries (optionally via local headers when central-directory reading fails)
//! - Creating new ZIPs via a manual writer that can enforce structure constraints for validators
//! - "Fake write" mode for building ZIP bytes in-memory without touching the filesystem
//!
//! For validator-oriented formats (e.g. TorrentZip-style rules), the writer path maintains
//! additional invariants such as filename ordering, timestamp normalization, and compression
//! method constraints.
include!("zip_file/impl.rs");

#[cfg(test)]
#[path = "tests/zip_file_tests.rs"]
mod tests;
