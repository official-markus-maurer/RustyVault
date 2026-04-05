/// Core status enums for the file tree nodes.
///
/// These enums define the primary state machine used during scanning, status aggregation,
/// and fix planning.
///
/// `ToSortDirType` is represented as a `bitflags` set for type-safe bitwise operations.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum RepStatus {
    // Scanning Status:
    /// Error state
    Error,
    /// Unset state
    UnSet,

    /// Initial missing state
    UnScanned,

    /// Directory matches correctly
    DirCorrect,
    /// Directory is missing
    DirMissing,
    /// Directory is not part of a DAT
    DirUnknown,
    /// Directory is in ToSort
    DirInToSort,
    /// Directory contains corrupted elements
    DirCorrupt,

    /// Expected but missing
    Missing, // a files or directory from a DAT that we do not have
    /// Verified and correct
    Correct, // a files or directory from a DAT that we have
    /// Not expected to be collected
    NotCollected, // a file from a DAT that is not collected that we do not have (either a merged or bad file.)
    /// Unneeded file
    UnNeeded, // a file from a DAT that is not collected that we do have, and so do not need. (a merged file in a child set)
    /// Unknown status
    Unknown, // a file that is not in a DAT
    /// File is in ToSort directory
    InToSort, // a file that is in the ToSort directory

    /// Corrupt archive
    Corrupt, // either a Zip file that is corrupt, or a Zipped file that is corrupt.
    /// Ignored file
    Ignore, // a file found in the ignore list

    // Fix Status:
    /// File can be repaired from another location
    CanBeFixed, // a missing file that can be fixed from another file. (Will be set to correct once it has been corrected)
    /// Move file to ToSort
    MoveToSort, // a file that is not in any DAT (Unknown) and should be moved to ToSort
    /// File can be deleted
    Delete, // a file that can be deleted
    /// Unknown file needed for a fix
    NeededForFix, // a file that is Unknown where it is, but is needed with out renaming to be placed in a Zip.
    /// Unknown file needed for a fix with a rename
    Rename, // a file that is Unknown where it is, but is needed with out renaming to be placed in a Zip.

    /// Corrupt file that can be replaced
    CorruptCanBeFixed, // a corrupt file that can be replaced and fixed from another file.
    /// Corrupt file to be moved
    MoveToCorrupt, // a corrupt file that should just be moved out the way so it can be fixed.

    /// Temporary value during fix
    Deleted, // this is a temporary value used while fixing sets, this value should never been seen.

    /// Expected but missing (MIA)
    MissingMIA,
    /// Verified (MIA)
    CorrectMIA,
    /// Can be repaired (MIA)
    CanBeFixedMIA,

    /// Delete pending due to incomplete state
    IncompleteRemove,
    /// Incomplete transfer or fix
    Incomplete,

    /// Marker for enum boundary
    EndValue,
}

/// Aggregated repair status used for UI rendering and tree summarization.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ReportStatus {
    /// Status not yet computed
    Unknown,
    /// Branch contains missing files
    Missing,
    /// Branch is fully verified
    Correct,
    /// Branch is ignored
    NotCollected,
    /// Branch contains only unneeded files
    UnNeeded,
    /// Branch is in ToSort
    InToSort,
    /// Branch contains corrupted files
    Corrupt,
    /// Explicitly ignored by user
    Ignore,
}

impl ReportStatus {
    /// Returns true if this branch is completely correct.
    pub fn has_correct(&self) -> bool {
        matches!(self, ReportStatus::Correct)
    }

    /// Returns true if this branch contains missing files.
    pub fn has_missing(&self, _b: bool) -> bool {
        matches!(self, ReportStatus::Missing | ReportStatus::Corrupt)
    }

    /// Returns true if this branch contains files that can be fixed.
    pub fn has_fixes_needed(&self) -> bool {
        matches!(self, ReportStatus::InToSort | ReportStatus::UnNeeded)
    }

    /// Returns true if this branch contains Missing-In-Action files.
    pub fn has_mia(&self) -> bool {
        matches!(self, ReportStatus::InToSort)
    }

    /// Returns true if all files in this branch are merged.
    pub fn has_all_merged(&self) -> bool {
        matches!(self, ReportStatus::NotCollected | ReportStatus::UnNeeded)
    }

    /// Returns true if the status of this branch is unknown.
    pub fn has_unknown(&self) -> bool {
        matches!(self, ReportStatus::Unknown | ReportStatus::Ignore)
    }
}

bitflags::bitflags! {
    /// Represents the categorization of a directory within the ToSort branch.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
    pub struct ToSortDirType: u8 {
        /// Standard directory
        const NONE = 0x00;
        /// Root ToSort directory
        const TO_SORT_PRIMARY = 0x01;
        /// Contains cache files
        const TO_SORT_CACHE = 0x02;
        /// Contains files only
        const TO_SORT_FILE_ONLY = 0x04;
    }
}

#[cfg(test)]
#[path = "tests/enums_tests.rs"]
mod tests;
