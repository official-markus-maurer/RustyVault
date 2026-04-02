bitflags::bitflags! {
    /// Bitflags representing the health and format state of a zip archive.
    /// 
    /// `TrrntZipStatus` maps exactly to the C# `TrrntZipStatus` bitfield enum. It is used
    /// by the `TorrentZipCheck` engine to accumulate all the structural flaws found inside 
    /// a standard zip file before attempting to rebuild it.
    /// 
    /// Differences from C#:
    /// - Utilizes the Rust `bitflags` crate to enforce type safety over raw integer masking,
    ///   preventing invalid status combinations.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TrrntZipStatus: u32 {
        const UNKNOWN = 0;
        const VALID_TRRNTZIP = 1;
        const CORRUPT_ZIP = 2;
        const UNSUPPORTED_COMPRESSION = 4;
        
        // Detailed check failures that require rebuilding
        const BAD_DIRECTORY_SEPARATOR = 8;
        const UNSORTED = 16;
        const EXTRA_DIRECTORY_ENTRIES = 32;
        const REPEAT_FILES_FOUND = 64;
        const BAD_COMPRESSION_METHOD = 128;
        const BAD_DATE_TIME = 256;
        const BAD_EXTRA_DATA = 512;
        const BAD_ZIP_COMMENT = 1024;
        const FILE_NAME_CASE_ERROR = 2048;
        
        // File access errors
        const SOURCE_FILE_LOCKED = 4096;
        const CATCH_ERROR = 8192;
        const USER_ABORTED = 16384;
        const USER_ABORTED_HARD = 32768;
    }
}
