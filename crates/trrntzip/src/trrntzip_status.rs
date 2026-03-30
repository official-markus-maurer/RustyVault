bitflags::bitflags! {
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
    }
}
