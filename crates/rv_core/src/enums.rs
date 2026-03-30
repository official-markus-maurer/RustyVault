#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum RepStatus {
    // Scanning Status:
    Error,

    UnSet,

    UnScanned,

    DirCorrect,
    DirMissing,
    DirUnknown,
    DirInToSort,
    DirCorrupt,

    Missing, // a files or directory from a DAT that we do not have
    Correct, // a files or directory from a DAT that we have
    NotCollected, // a file from a DAT that is not collected that we do not have (either a merged or bad file.)
    UnNeeded, // a file from a DAT that is not collected that we do have, and so do not need. (a merged file in a child set)
    Unknown, // a file that is not in a DAT
    InToSort, // a file that is in the ToSort directory

    Corrupt, // either a Zip file that is corrupt, or a Zipped file that is corrupt
    Ignore, // a file found in the ignore list

    // Fix Status:
    CanBeFixed, // a missing file that can be fixed from another file. (Will be set to correct once it has been corrected)
    MoveToSort, // a file that is not in any DAT (Unknown) and should be moved to ToSort
    Delete, // a file that can be deleted 
    NeededForFix, // a file that is Unknown where it is, but is needed somewhere else.
    Rename, // a file that is Unknown where it is, but is needed with other name inside the same Zip.

    CorruptCanBeFixed, // a corrupt file that can be replaced and fixed from another file.
    MoveToCorrupt, // a corrupt file that should just be moved out the way to a corrupt directory in ToSort.

    Deleted, // this is a temporary value used while fixing sets, this value should never been seen.

    MissingMIA,
    CorrectMIA,
    CanBeFixedMIA,

    IncompleteRemove,
    Incomplete,

    EndValue
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum ReportStatus {
    Unknown,
    Missing,
    Correct,
    NotCollected,
    UnNeeded,
    InToSort,
    Corrupt,
    Ignore,
}

impl ReportStatus {
    pub fn has_correct(&self) -> bool {
        matches!(self, ReportStatus::Correct)
    }

    pub fn has_missing(&self, _b: bool) -> bool {
        matches!(self, ReportStatus::Missing)
    }

    pub fn has_fixes_needed(&self) -> bool {
        false // Simplified for now
    }

    pub fn has_mia(&self) -> bool {
        false // Simplified for now
    }

    pub fn has_all_merged(&self) -> bool {
        matches!(self, ReportStatus::NotCollected)
    }

    pub fn has_unknown(&self) -> bool {
        matches!(self, ReportStatus::Unknown)
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
    pub struct ToSortDirType: u8 {
        const NONE = 0x00;
        const TO_SORT_PRIMARY = 0x01;
        const TO_SORT_CACHE = 0x02;
        const TO_SORT_FILE_ONLY = 0x04;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_status_methods() {
        assert!(ReportStatus::Correct.has_correct());
        assert!(!ReportStatus::Missing.has_correct());

        assert!(ReportStatus::Missing.has_missing(false));
        assert!(!ReportStatus::Correct.has_missing(false));

        assert!(ReportStatus::NotCollected.has_all_merged());
        assert!(!ReportStatus::Correct.has_all_merged());

        assert!(ReportStatus::Unknown.has_unknown());
        assert!(!ReportStatus::Correct.has_unknown());
    }

    #[test]
    fn test_tosortdirtype_bitflags() {
        let mut flags = ToSortDirType::NONE;
        flags.insert(ToSortDirType::TO_SORT_PRIMARY);
        flags.insert(ToSortDirType::TO_SORT_CACHE);
        
        assert!(flags.contains(ToSortDirType::TO_SORT_PRIMARY));
        assert!(flags.contains(ToSortDirType::TO_SORT_CACHE));
        assert!(!flags.contains(ToSortDirType::TO_SORT_FILE_ONLY));

        flags.remove(ToSortDirType::TO_SORT_PRIMARY);
        assert!(!flags.contains(ToSortDirType::TO_SORT_PRIMARY));
    }
}
