use std::env;
use std::fs::File;
use rv_core::scanner::Scanner;
use dat_reader::enums::FileType;
use dat_reader::dat_store::{DatHeader, DatNode, DatGame};
use dat_reader::xml_writer::DatXmlWriter;

/// CLI tool to generate a standard XML DAT file from a physical directory.
/// 
/// `dir2dat` mimics the functionality of the C# `RomVault` `Dir2Dat` context menu option.
/// It scans a directory, dives into zip/7z archives, calculates their hashes, and exports
/// a `.dat` XML describing the folder's structure perfectly.
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Dir2Dat v0.1.0 - Powered by RustyRoms");
        println!("");
        println!("Usage: dir2dat <directory_path>");
        println!("This tool scans a physical directory and generates a standard XML DAT file representing its contents.");
        return;
    }

    let target_dir = &args[1];
    println!("Scanning directory: {}", target_dir);

    // Scan the physical directory into ScannedFiles
    let scanned_files = Scanner::scan_directory(target_dir);

    // Initialize the root DAT header
    let mut dat_header = DatHeader {
        name: Some("Dir2Dat Output".to_string()),
        description: Some(format!("Generated from {}", target_dir)),
        author: Some("RustyRoms Dir2Dat".to_string()),
        version: Some("1.0".to_string()),
        ..Default::default()
    };

    // Convert ScannedFiles into DatNodes
    for file in scanned_files {
        // If it's an archive, we deep scan it to get the files inside
        if file.file_type == FileType::Zip || file.file_type == FileType::SevenZip {
            println!("Deep scanning archive: {}", file.name);
            match Scanner::scan_archive_file(file.file_type, &format!("{}/{}", target_dir, file.name), 0, true) {
                Ok(arch) => {
                    let mut d_dir = DatNode::new_dir(file.name.clone(), file.file_type);
                    if let Some(d) = d_dir.dir_mut() {
                        let mut game = DatGame::default();
                        game.description = Some(file.name.clone());
                        d.d_game = Some(Box::new(game));

                        for child in arch.children {
                            let mut f_node = DatNode::new_file(child.name, child.file_type);
                            if let Some(f) = f_node.file_mut() {
                                f.size = child.size;
                                f.crc = child.crc;
                                f.sha1 = child.sha1;
                                f.md5 = child.md5;
                            }
                            d.add_child(f_node);
                        }
                    }
                    dat_header.base_dir.add_child(d_dir);
                }
                Err(e) => {
                    println!("Error scanning archive {}: {:?}", file.name, e);
                }
            }
        } else if file.file_type == FileType::File {
            // It's a raw file, treat it as a game with one ROM
            let mut d_dir = DatNode::new_dir(file.name.clone(), FileType::Dir);
            if let Some(d) = d_dir.dir_mut() {
                let mut game = DatGame::default();
                game.description = Some(file.name.clone());
                d.d_game = Some(Box::new(game));

                println!("Deep scanning raw file: {}", file.name);
                let full_path = format!("{}/{}", target_dir, file.name);
                let mut f_node = DatNode::new_file(file.name.clone(), file.file_type);
                
                match Scanner::scan_raw_file(&full_path) {
                    Ok(scanned_raw) => {
                        if let Some(f) = f_node.file_mut() {
                            f.size = scanned_raw.size;
                            f.crc = scanned_raw.crc;
                            f.sha1 = scanned_raw.sha1;
                            f.md5 = scanned_raw.md5;
                        }
                    }
                    Err(e) => {
                        println!("Failed to read raw file {}: {:?}", file.name, e);
                        if let Some(f) = f_node.file_mut() {
                            f.size = file.size; // fallback to basic size
                        }
                    }
                }
                d.add_child(f_node);
            }
            dat_header.base_dir.add_child(d_dir);
        }
    }

    let out_file = "output.dat";
    println!("Writing DAT file to {}", out_file);
    
    if let Ok(mut file) = File::create(out_file) {
        if let Err(e) = DatXmlWriter::write_dat(&mut file, &dat_header) {
            println!("Failed to write DAT file: {}", e);
        } else {
            println!("Successfully generated {}", out_file);
        }
    } else {
        println!("Failed to create output file.");
    }
}
