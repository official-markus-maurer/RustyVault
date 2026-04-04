use crate::cmp_reader::DatFileLoader;
use crate::dat_store::{DatDir, DatGame, DatHeader, DatNode};
use crate::enums::FileType;
use crate::var_fix;

/// DOSCenter DAT parser.
///
/// `dos_reader.rs` parses the legacy DOSCenter DAT format, which structurally resembles
/// ClrMamePro (CMP) but uses parentheses `( )` instead of curly braces for blocks.
///
/// Differences from C#:
/// - Like `rom_center_reader`, this heavily leverages the zero-copy `DatFileLoader`
///   tokenizer from `cmp_reader.rs` for extreme speed and memory efficiency compared
///   to standard C# `StreamReader` loops.
pub fn read_dos_dat(input: &str, filename: &str) -> Result<DatHeader, String> {
    let mut dfl = DatFileLoader::new(input);
    let mut dat_header = DatHeader {
        base_dir: DatDir::new(FileType::Dir),
        filename: Some(filename.to_string()),
        ..Default::default()
    };

    dfl.gn();
    if dfl.end_of_stream() {
        return Err("Empty DOS DAT".to_string());
    }

    if dfl.next_token.to_lowercase() == "doscenter" {
        dfl.gn();
        load_header(&mut dfl, &mut dat_header)?;
        dfl.gn();
    }

    while !dfl.end_of_stream() {
        if dfl.next_token.to_lowercase() == "game" {
            dfl.gn();
            load_game(&mut dfl, &mut dat_header.base_dir)?;
            dfl.gn();
        } else {
            dfl.gn();
        }
    }

    Ok(dat_header)
}

fn load_header(dfl: &mut DatFileLoader, dat_header: &mut DatHeader) -> Result<(), String> {
    if dfl.next_token != "(" {
        return Err("Expected ( after DOSCenter".to_string());
    }
    dfl.gn();

    dat_header.compression = Some("TDC".to_string());

    while dfl.next_token != ")" && !dfl.end_of_stream() {
        let nextstr = dfl.next_token.to_lowercase();

        if let Some(stripped) = nextstr.strip_prefix("name:") {
            let rest = dfl.gn_rest();
            dat_header.name = Some(format!("{} {}", stripped, rest).trim().to_string());
            dfl.gn();
        } else {
            match nextstr.as_str() {
                "name" | "name:" => {
                    dat_header.name = Some(dfl.gn_rest());
                    dfl.gn();
                }
                "description" | "description:" => {
                    dat_header.description = Some(dfl.gn_rest());
                    dfl.gn();
                }
                "version" | "version:" => {
                    dat_header.version = Some(dfl.gn_rest());
                    dfl.gn();
                }
                "date" | "date:" => {
                    dat_header.date = Some(dfl.gn_rest());
                    dfl.gn();
                }
                "author" | "author:" => {
                    dat_header.author = Some(dfl.gn_rest());
                    dfl.gn();
                }
                "homepage" | "homepage:" => {
                    dat_header.homepage = Some(dfl.gn_rest());
                    dfl.gn();
                }
                "comment" | "comment:" => {
                    dat_header.comment = Some(dfl.gn_rest());
                    dfl.gn();
                }
                _ => {
                    dfl.gn();
                }
            }
        }
    }
    Ok(())
}

fn load_game(dfl: &mut DatFileLoader, parent_dir: &mut DatDir) -> Result<(), String> {
    if dfl.next_token != "(" {
        return Err("Expected ( after game".to_string());
    }
    dfl.gn();

    if dfl.next_token.to_lowercase() != "name" {
        return Err("Name not found first".to_string());
    }

    let mut name = dfl.gn_rest();
    if name.to_lowercase().ends_with(".zip") {
        name = name[..name.len() - 4].to_string();
    }
    dfl.gn();

    let mut d_dir = DatNode::new_dir(name, FileType::UnSet);
    if let Some(d) = d_dir.dir_mut() {
        d.d_game = Some(Box::new(DatGame::default()));
    }

    while dfl.next_token != ")" && !dfl.end_of_stream() {
        match dfl.next_token.to_lowercase().as_str() {
            "file" | "rom" => {
                dfl.gn();
                load_file(dfl, d_dir.dir_mut().unwrap())?;
                dfl.gn();
            }
            _ => {
                dfl.gn();
            }
        }
    }

    parent_dir.add_child(d_dir);
    Ok(())
}

fn load_file(dfl: &mut DatFileLoader, parent_dir: &mut DatDir) -> Result<(), String> {
    if dfl.next_token != "(" {
        return Err("Expected ( after file".to_string());
    }
    dfl.gn();

    if dfl.next_token.to_lowercase() != "name" {
        return Err("Name not found first in file".to_string());
    }

    // GnNameToSize equivalent
    // In DOSCenter dat files, filenames can contain spaces. We read everything up to "size".
    // For simplicity here, we assume standard tokenization works well enough if there are no spaces,
    // or we'd need to extend DatFileLoader to support `gn_name_to_size`.
    // Implementing a basic fallback:
    let mut name = String::new();
    loop {
        let t = dfl.gn();
        if t.to_lowercase() == "size" {
            break;
        }
        if !name.is_empty() {
            name.push(' ');
        }
        name.push_str(&t);
        if dfl.end_of_stream() || dfl.next_token == ")" {
            break;
        }
    }

    let mut d_rom = DatNode::new_file(name.trim().to_string(), FileType::UnSet);

    let size_value = dfl.gn();
    if let Some(f) = d_rom.file_mut() {
        f.size = var_fix::u64_opt(&size_value);
    }
    dfl.gn();

    while dfl.next_token != ")" && !dfl.end_of_stream() {
        match dfl.next_token.to_lowercase().as_str() {
            "crc" => {
                if let Some(f) = d_rom.file_mut() {
                    f.crc = var_fix::clean_md5_sha1(&dfl.gn(), 8);
                }
                dfl.gn();
            }
            "sha1" => {
                if let Some(f) = d_rom.file_mut() {
                    f.sha1 = var_fix::clean_md5_sha1(&dfl.gn(), 40);
                }
                dfl.gn();
            }
            "date" => {
                let _d = dfl.gn();
                let _t = dfl.gn();
                // skip parsing date to ticks for now
                dfl.gn();
            }
            _ => {
                dfl.gn();
            }
        }
    }

    parent_dir.add_child(d_rom);
    Ok(())
}
