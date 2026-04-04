use super::*;

#[test]
fn test_render_repair_report_includes_not_collected_line() {
    let mut report = RepairStatus::new();
    report.total_roms = 7;
    report.roms_correct = 2;
    report.roms_correct_mia = 1;
    report.roms_missing = 1;
    report.roms_fixes = 1;
    report.roms_unknown = 2;
    report.roms_not_collected = 2;
    report.roms_unneeded = 1;

    let lines = render_repair_report(&report);

    assert!(lines.iter().any(|line| line == "Correct:    2"));
    assert!(lines.iter().any(|line| line == "Missing:    2"));
    assert!(lines.iter().any(|line| line == "Can Fix:    4"));
    assert!(lines.iter().any(|line| line == "Not Collected: 2"));
    assert!(lines.iter().any(|line| line == "Unneeded:   1"));
}
