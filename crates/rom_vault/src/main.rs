use std::io::{self, Write};
use rv_core::db::{init_db, GLOBAL_DB};
use rv_core::scanner::Scanner;
use rv_core::find_fixes::FindFixes;
use rv_core::fix::Fix;
use rv_core::read_dat::DatUpdate;
use rv_core::repair_status::RepairStatus;
use rv_core::settings::{get_settings, update_settings};
use dat_reader::enums::FileType;

fn main() {
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
                    },
                    "update_dats" => {
                        if parts.len() < 2 {
                            println!("Usage: update_dats <directory_path>");
                        } else {
                            let path = parts[1..].join(" ");
                            DatUpdate::update_dat(root.clone(), &path);
                        }
                    },
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
                            let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
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
                            rv_core::file_scanning::FileScanning::scan_dir(target_node, &mut root_scan);
                            println!("Integration complete.");
                        }
                    },
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
                                    println!("Archive scanned successfully. Comment: '{}'", arch.comment);
                                    println!("Found {} files inside:", arch.children.len());
                                    for c in arch.children {
                                        println!(" - {} (size: {:?})", c.name, c.size);
                                    }
                                },
                                Err(e) => println!("Error scanning archive: {:?}", e),
                            }
                        }
                    },
                    "find_fixes" => {
                        println!("Running FindFixes on current DB tree...");
                        FindFixes::scan_files(root.clone());
                        println!("FindFixes completed.");
                    },
                    "fix" => {
                        println!("Performing physical fixes (Move/Rename/Delete)...");
                        Fix::perform_fixes(root.clone());
                        println!("Fixes completed.");
                    },
                    "report" => {
                        let mut report = RepairStatus::new();
                        report.report_status(root.clone());
                        println!("--- Repair Report ---");
                        println!("Total ROMs: {}", report.total_roms);
                        println!("Correct:    {}", report.roms_correct);
                        println!("Missing:    {}", report.roms_missing);
                        println!("Can Fix:    {}", report.roms_fixes);
                        println!("Unneeded:   {}", report.roms_unneeded);
                        println!("Unknown:    {}", report.roms_unknown);
                        println!("---------------------");
                    },
                    "fixdat" => {
                        if parts.len() < 2 {
                            println!("Usage: fixdat <out_directory>");
                        } else {
                            let path = parts[1..].join(" ");
                            println!("Generating Fix DATs in: {}", path);
                            // Ensure the output directory exists
                            let _ = std::fs::create_dir_all(&path);
                            rv_core::fix_dat_report::FixDatReport::recursive_dat_tree(&path, root.clone(), false);
                            println!("Fix DAT generation complete.");
                        }
                    },
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
                                    println!("      [{}] {} (Dats: {})", j, cd.name, cd.dir_dats.len());
                                }
                            }
                        }
                    },
                    _ => {
                        println!("Unknown command. Type 'help' for a list of commands.");
                    }
                }
            }
        }
    });

    println!("Shutting down RustyRoms.");
}
