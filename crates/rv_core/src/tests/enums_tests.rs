use super::*;

#[test]
fn test_report_status_methods() {
    assert!(ReportStatus::Correct.has_correct());
    assert!(!ReportStatus::Missing.has_correct());

    assert!(ReportStatus::Missing.has_missing(false));
    assert!(ReportStatus::Corrupt.has_missing(false));
    assert!(!ReportStatus::Correct.has_missing(false));

    assert!(ReportStatus::InToSort.has_fixes_needed());
    assert!(ReportStatus::UnNeeded.has_fixes_needed());
    assert!(!ReportStatus::Correct.has_fixes_needed());

    assert!(ReportStatus::InToSort.has_mia());
    assert!(!ReportStatus::Correct.has_mia());

    assert!(ReportStatus::NotCollected.has_all_merged());
    assert!(ReportStatus::UnNeeded.has_all_merged());
    assert!(!ReportStatus::Correct.has_all_merged());

    assert!(ReportStatus::Unknown.has_unknown());
    assert!(ReportStatus::Ignore.has_unknown());
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
