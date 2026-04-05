use dat_reader::enums::FileType;
use rv_core::db::{init_db, GLOBAL_DB};
use rv_core::file_scanning::FileScanning;
use rv_core::find_fixes::FindFixes;
use rv_core::fix::Fix;
use rv_core::read_dat::DatUpdate;
use rv_core::repair_status::RepairStatus;
use rv_core::rv_file::RvFile;
use rv_core::scanned_file::ScannedFile;
use rv_core::scanner::Scanner;
use rv_core::settings::EScanLevel;
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

fn render_repair_report(report: &RepairStatus) -> Vec<String> {
    vec![
        "--- Repair Report ---".to_string(),
        format!("Total ROMs: {}", report.total_roms),
        format!("Correct:    {}", report.count_correct()),
        format!("Missing:    {}", report.count_missing()),
        format!("Can Fix:    {}", report.count_fixes_needed()),
        format!("Not Collected: {}", report.roms_not_collected),
        format!("Unneeded:   {}", report.roms_unneeded),
        format!("Unknown:    {}", report.roms_unknown),
        "---------------------".to_string(),
    ]
}

fn recompute_fix_plan(root: Rc<RefCell<RvFile>>) {
    FindFixes::scan_files(Rc::clone(&root));
    RepairStatus::report_status_reset(root);
}

fn is_actionable_fix_status(status: rv_core::enums::RepStatus) -> bool {
    matches!(
        status,
        rv_core::enums::RepStatus::CanBeFixed
            | rv_core::enums::RepStatus::CanBeFixedMIA
            | rv_core::enums::RepStatus::CorruptCanBeFixed
            | rv_core::enums::RepStatus::UnNeeded
            | rv_core::enums::RepStatus::MoveToSort
            | rv_core::enums::RepStatus::MoveToCorrupt
            | rv_core::enums::RepStatus::Rename
            | rv_core::enums::RepStatus::Delete
            | rv_core::enums::RepStatus::IncompleteRemove
    )
}

fn count_selected_actionable_fixes(node: Rc<RefCell<RvFile>>) -> i32 {
    let (is_selected, rep_status, is_dir, children) = {
        let n = node.borrow();
        (
            matches!(
                n.tree_checked,
                rv_core::rv_file::TreeSelect::Selected | rv_core::rv_file::TreeSelect::Locked
            ),
            n.rep_status(),
            n.is_directory(),
            n.children.clone(),
        )
    };

    let mut count = if is_selected && is_actionable_fix_status(rep_status) {
        1
    } else {
        0
    };

    if is_dir {
        for child in children {
            count += count_selected_actionable_fixes(child);
        }
    }

    count
}

fn current_fixable_count(root: Rc<RefCell<RvFile>>) -> i32 {
    count_selected_actionable_fixes(root)
}

fn branch_has_selected_nodes(node: &RvFile) -> bool {
    if matches!(
        node.tree_checked,
        rv_core::rv_file::TreeSelect::Selected | rv_core::rv_file::TreeSelect::Locked
    ) {
        return true;
    }

    for child in &node.children {
        if branch_has_selected_nodes(&child.borrow()) {
            return true;
        }
    }

    false
}

fn rescan_selected_roots(root: Rc<RefCell<RvFile>>, scan_level: EScanLevel) {
    let root_children = root.borrow().children.clone();

    for child in root_children {
        let (name, is_selected) = {
            let node = child.borrow();
            (node.name.clone(), branch_has_selected_nodes(&node))
        };

        if !is_selected {
            continue;
        }

        let physical_path = rv_core::settings::find_dir_mapping(&name).unwrap_or(name.clone());
        let rule = rv_core::settings::find_rule(&name);
        let files = Scanner::scan_directory_with_level_and_ignore(
            &physical_path,
            scan_level,
            &rule.ignore_files.items,
        );
        let mut root_scan = ScannedFile::new(FileType::Dir);
        root_scan.name = name.clone();
        root_scan.children = files;
        FileScanning::scan_dir_with_level(Rc::clone(&child), &mut root_scan, scan_level);
    }

    RepairStatus::report_status_reset(root);
}

/// Headless CLI binary for the RomVault engine.
///
/// `rom_vault` provides a fully text-based command line interface to interact with the
/// core `RustyVault` engine (`rv_core`) without needing to launch the `egui` desktop application.
fn main() {
    rv_core::settings::load_settings_from_file();
    let settings = rv_core::settings::get_settings();
    if settings.debug_logs_enabled {
        let file = rv_core::open_update_log_file();
        if let Some(file) = file {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .with_writer(move || file.try_clone().unwrap())
                .try_init();
        } else {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .try_init();
        }
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();
    }

    println!("Welcome to RustyVault v0.1.0");

    // Initialize the DB
    init_db();
    println!("Database initialized with default root structure.");

    GLOBAL_DB.with(|db_ref| {
        if let Some(db) = db_ref.borrow().as_ref() {
            let root = db.dir_root.clone();

            // Basic CLI loop
            loop {
                print!("> ");
                io::stdout().flush().unwrap();

                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                let cmd = input.trim().to_lowercase();
                let parts: Vec<&str> = cmd.split_whitespace().collect();

                if parts.is_empty() {
                    continue;
                }

                match parts[0] {
                    "exit" | "quit" => break,
                    "help" => {
                        println!("Available commands:");
                        println!("  update_dats <dir> - Read DAT files and build expected tree");
                        println!("  scan <dir>        - Scan a directory on disk");
                        println!("  scan_arch <file>  - Deep scan an archive (.zip/.7z)");
                        println!("  find_fixes        - Run FindFixes logic to identify matches");
                        println!("  fix               - Perform actual file moving/fixing");
                        println!("  report            - Generate Repair Status report");
                        println!("  fixdat <out_dir>  - Generate a Fix DAT of missing files");
                        println!("  status            - Show current DB root tree status");
                        println!("  exit/quit         - Exit the application");
                    }
                    "update_dats" => {
                        if parts.len() < 2 {
                            println!("Usage: update_dats <directory_path>");
                        } else {
                            let path = parts[1..].join(" ");
                            DatUpdate::update_dat(root.clone(), &path);
                        }
                    }
                    "scan" => {
                        if parts.len() < 2 {
                            println!("Usage: scan <directory_path>");
                        } else {
                            let path = parts[1..].join(" ");
                            println!("Scanning directory: {}", path);
                            let files = Scanner::scan_directory(&path);
                            println!("Found {} files/folders.", files.len());

                            // Mocking the top level integration
                            // We construct a mock ScannedFile root to hold the results
                            let mut root_scan =
                                rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                            root_scan.children = files;

                            // To properly scan against the root we'd need to match the node
                            // For CLI simplicity, we just assume they want to scan RustyVault
                            let mut target_node = root.clone();
                            for child in root.borrow().children.iter() {
                                if child.borrow().name == path {
                                    target_node = child.clone();
                                    break;
                                }
                            }

                            println!("Integrating scanned files into database tree...");
                            rv_core::file_scanning::FileScanning::scan_dir(
                                target_node,
                                &mut root_scan,
                            );
                            println!("Integration complete.");
                        }
                    }
                    "scan_arch" => {
                        if parts.len() < 2 {
                            println!("Usage: scan_arch <file_path>");
                        } else {
                            let path = parts[1..].join(" ");
                            let file_type = if path.ends_with(".zip") {
                                FileType::Zip
                            } else if path.ends_with(".7z") {
                                FileType::SevenZip
                            } else {
                                FileType::File
                            };

                            println!("Deep scanning archive: {}", path);
                            match Scanner::scan_archive_file(file_type, &path, 0, true) {
                                Ok(arch) => {
                                    println!(
                                        "Archive scanned successfully. Comment: '{}'",
                                        arch.comment
                                    );
                                    println!("Found {} files inside:", arch.children.len());
                                    for c in arch.children {
                                        println!(" - {} (size: {:?})", c.name, c.size);
                                    }
                                }
                                Err(e) => println!("Error scanning archive: {:?}", e),
                            }
                        }
                    }
                    "find_fixes" => {
                        println!("Running FindFixes on current DB tree...");
                        recompute_fix_plan(root.clone());
                        println!("FindFixes completed.");
                    }
                    "fix" => {
                        println!("Refreshing DB from disk...");
                        rescan_selected_roots(root.clone(), EScanLevel::Level2);
                        for pass in 1..=4 {
                            println!("Refreshing fix plan (pass {pass}/4)...");
                            recompute_fix_plan(root.clone());
                            let pending = current_fixable_count(root.clone());
                            if pending == 0 {
                                break;
                            }
                            println!("Performing physical fixes (pass {pass}/4)...");
                            Fix::perform_fixes(root.clone());
                            println!("Refreshing DB after fix pass {pass}/4...");
                            rescan_selected_roots(root.clone(), EScanLevel::Level2);
                        }
                        println!("Refreshing final repair state...");
                        recompute_fix_plan(root.clone());
                        println!("Fixes completed.");
                    }
                    "report" => {
                        let mut report = RepairStatus::new();
                        report.report_status(root.clone());
                        for line in render_repair_report(&report) {
                            println!("{line}");
                        }
                    }
                    "fixdat" => {
                        if parts.len() < 2 {
                            println!("Usage: fixdat <out_directory>");
                        } else {
                            let path = parts[1..].join(" ");
                            println!("Generating Fix DATs in: {}", path);
                            // Ensure the output directory exists
                            let _ = std::fs::create_dir_all(&path);
                            rv_core::fix_dat_report::FixDatReport::recursive_dat_tree(
                                &path,
                                root.clone(),
                                false,
                            );
                            println!("Fix DAT generation complete.");
                        }
                    }
                    "status" => {
                        let root_borrow = root.borrow();
                        println!("DB Root Node: {} children", root_borrow.children.len());
                        for (i, child) in root_borrow.children.iter().enumerate() {
                            let c = child.borrow();
                            println!("  [{}] {} (Status: {:?})", i, c.name, c.dat_status());
                            if c.name == "RustyVault" {
                                println!("    DATs attached: {}", c.dir_dats.len());
                                for (j, d) in c.children.iter().enumerate() {
                                    let cd = d.borrow();
                                    println!(
                                        "      [{}] {} (Dats: {})",
                                        j,
                                        cd.name,
                                        cd.dir_dats.len()
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        println!("Unknown command. Type 'help' for a list of commands.");
                    }
                }
            }
        }
    });

    println!("Shutting down RustyRoms.");
}

#[cfg(test)]
#[path = "tests/main_tests.rs"]
mod tests;
