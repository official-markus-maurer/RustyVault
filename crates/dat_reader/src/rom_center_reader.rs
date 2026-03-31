use crate::dat_store::{DatDir, DatGame, DatHeader, DatNode};
use crate::enums::FileType;
use crate::cmp_reader::DatFileLoader;

/// RomCenter DAT parser.
/// 
/// `rom_center_reader.rs` parses the legacy RomCenter INI-style text DAT format
/// (sections denoted by `[games]`, `[credits]`, etc.). 
/// 
/// Differences from C#:
/// - Re-uses the zero-copy `DatFileLoader` tokenizer originally built for `cmp_reader.rs`,
///   allowing it to share the same highly optimized string slicing logic rather than
///   relying on generic `StreamReader.ReadLine()` calls.
pub fn read_rom_center_dat(input: &str, filename: &str) -> Result<DatHeader, String> {
    let mut dfl = DatFileLoader::new(input);
    let mut dat_header = DatHeader {
        base_dir: DatDir::new(),
        filename: Some(filename.to_string()),
        ..Default::default()
    };

    dfl.gn();

    while !dfl.end_of_stream() {
        let token = dfl.next_token.to_lowercase();
        match token.as_str() {
            "[credits]" => load_credits(&mut dfl, &mut dat_header)?,
            "[dat]" => load_dat(&mut dfl, &mut dat_header)?,
            "[emulator]" => load_emulator(&mut dfl, &mut dat_header)?,
            "[games]" | "[resources]" => load_game(&mut dfl, &mut dat_header.base_dir)?,
            "[disks]" => load_disks(&mut dfl, &mut dat_header.base_dir)?,
            _ => {
                return Err(format!("Unknown section {}", token));
            }
        }
    }

    Ok(dat_header)
}

fn split_line(s: &str) -> Option<(String, String)> {
    let idx = s.find('=')?;
    let element = s[..idx].to_string();
    let value = s[idx + 1..].to_string();
    Some((element, value))
}

fn load_credits(dfl: &mut DatFileLoader, dat_header: &mut DatHeader) -> Result<(), String> {
    while !dfl.end_of_stream() {
        let line = dfl.gn();
        if line.starts_with('[') {
            return Ok(());
        }

        if let Some((element, value)) = split_line(&line) {
            match element.to_lowercase().as_str() {
                "author" => dat_header.author = Some(value),
                "email" => dat_header.email = Some(value),
                "homepage" | "url" => dat_header.url = Some(value),
                "version" => dat_header.version = Some(value),
                "date" => dat_header.date = Some(value),
                "comment" => dat_header.comment = Some(value),
                _ => {}
            }
        }
    }
    Ok(())
}

fn load_dat(dfl: &mut DatFileLoader, dat_header: &mut DatHeader) -> Result<(), String> {
    while !dfl.end_of_stream() {
        let line = dfl.gn();
        if line.starts_with('[') {
            return Ok(());
        }

        if let Some((element, value)) = split_line(&line) {
            match element.to_lowercase().as_str() {
                "split" => dat_header.split = Some(value),
                "merge" => dat_header.merge_type = Some(value),
                _ => {}
            }
        }
    }
    Ok(())
}

fn load_emulator(dfl: &mut DatFileLoader, dat_header: &mut DatHeader) -> Result<(), String> {
    while !dfl.end_of_stream() {
        let line = dfl.gn();
        if line.starts_with('[') {
            return Ok(());
        }

        if let Some((element, value)) = split_line(&line) {
            match element.to_lowercase().as_str() {
                "refname" => dat_header.name = Some(value),
                "version" => dat_header.description = Some(value),
                "category" => dat_header.category = Some(value),
                _ => {}
            }
        }
    }
    Ok(())
}

fn load_game(dfl: &mut DatFileLoader, parent_dir: &mut DatDir) -> Result<(), String> {
    while !dfl.end_of_stream() {
        let line = dfl.gn();
        if line.starts_with('[') {
            return Ok(());
        }

        let parts: Vec<&str> = if line.contains('�') {
            line.split('�').collect()
        } else if line.contains('¬') {
            line.split('¬').collect()
        } else {
            continue;
        };

        if parts.len() < 10 {
            continue;
        }

        let parent_name = parts[1];
        let _parent_desc = parts[2];
        let game_name = parts[3];
        let game_desc = parts[4];
        let rom_name = parts[5];
        let rom_crc = parts[6];
        let rom_size = parts[7];
        let rom_of = parts[8];
        let merge = parts[9];

        // Find or create game directory
        let mut found_idx = None;
        for (i, child) in parent_dir.children.iter().enumerate() {
            if child.name == game_name && child.is_dir() {
                found_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = found_idx {
            if let Some(d) = parent_dir.children[idx].dir_mut() {
                let mut d_rom = DatNode::new_file(rom_name.to_string(), FileType::UnSet);
                if let Some(f) = d_rom.file_mut() {
                    f.crc = hex::decode(rom_crc).ok();
                    f.size = rom_size.parse::<u64>().ok();
                    f.merge = Some(merge.to_string());
                }
                d.add_child(d_rom);
            }
        } else {
            let mut d_dir = DatNode::new_dir(game_name.to_string(), FileType::UnSet);
            if let Some(d) = d_dir.dir_mut() {
                let mut d_game = DatGame::default();
                d_game.description = Some(game_desc.to_string());
                if parent_name != game_name {
                    d_game.clone_of = Some(parent_name.to_string());
                }
                if rom_of != game_name {
                    d_game.rom_of = Some(rom_of.to_string());
                }
                d.d_game = Some(Box::new(d_game));

                let mut d_rom = DatNode::new_file(rom_name.to_string(), FileType::UnSet);
                if let Some(f) = d_rom.file_mut() {
                    f.crc = hex::decode(rom_crc).ok();
                    f.size = rom_size.parse::<u64>().ok();
                    f.merge = Some(merge.to_string());
                }
                d.add_child(d_rom);
            }
            parent_dir.add_child(d_dir);
        }
    }
    Ok(())
}

fn load_disks(dfl: &mut DatFileLoader, parent_dir: &mut DatDir) -> Result<(), String> {
    while !dfl.end_of_stream() {
        let line = dfl.gn();
        if line.starts_with('[') {
            return Ok(());
        }

        let parts: Vec<&str> = if line.contains('�') {
            line.split('�').collect()
        } else if line.contains('¬') {
            line.split('¬').collect()
        } else {
            continue;
        };

        if parts.len() < 10 {
            continue;
        }

        let parent_name = parts[1];
        let game_name = parts[3];
        let game_desc = parts[4];
        let rom_name = parts[5];
        let rom_crc = parts[6]; // sha1 for chd in romcenter
        let merge = parts[9];

        let mut found_idx = None;
        for (i, child) in parent_dir.children.iter().enumerate() {
            if child.name == game_name && child.is_dir() {
                found_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = found_idx {
            if let Some(d) = parent_dir.children[idx].dir_mut() {
                let mut d_rom = DatNode::new_file(format!("{}.chd", rom_name), FileType::UnSet);
                if let Some(f) = d_rom.file_mut() {
                    f.is_disk = true;
                    f.sha1 = hex::decode(rom_crc).ok();
                    f.merge = Some(format!("{}.chd", merge));
                }
                d.add_child(d_rom);
            }
        } else {
            let mut d_dir = DatNode::new_dir(game_name.to_string(), FileType::UnSet);
            if let Some(d) = d_dir.dir_mut() {
                let mut d_game = DatGame::default();
                d_game.description = Some(game_desc.to_string());
                if parent_name != game_name {
                    d_game.clone_of = Some(parent_name.to_string());
                }
                d.d_game = Some(Box::new(d_game));

                let mut d_rom = DatNode::new_file(format!("{}.chd", rom_name), FileType::UnSet);
                if let Some(f) = d_rom.file_mut() {
                    f.is_disk = true;
                    f.sha1 = hex::decode(rom_crc).ok();
                    f.merge = Some(format!("{}.chd", merge));
                }
                d.add_child(d_rom);
            }
            parent_dir.add_child(d_dir);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_rom_center_dat() {
        let dat = r#"[CREDITS]
Author=Tester
Version=1.00
[DAT]
split=true
merge=split
[GAMES]
parentgame�desc1�mygame�My Game�rom1.bin�12345678�1024�romofgame�merge.bin�
[DISKS]
parentgame�desc1�mygame�My Game�mydisk�9876543210abcdef�size�romof�mergedisk�"#;

        let header = read_rom_center_dat(dat, "test.dat").unwrap();
        assert_eq!(header.author, Some("Tester".to_string()));
        assert_eq!(header.split, Some("true".to_string()));

        let games = &header.base_dir.children;
        assert_eq!(games.len(), 1);

        let game_node = &games[0];
        assert_eq!(game_node.name(), "mygame");

        let game_dir = game_node.dir().unwrap();
        let game_meta = game_dir.d_game.as_ref().unwrap();
        assert_eq!(game_meta.clone_of, Some("parentgame".to_string()));
        assert_eq!(game_meta.description, Some("My Game".to_string()));

        let files = &game_dir.children;
        assert_eq!(files.len(), 2);

        // Rom Check
        let rom = files[0].file().unwrap();
        assert_eq!(files[0].name(), "rom1.bin");
        assert_eq!(rom.size, Some(1024));
        assert_eq!(rom.crc, Some(hex::decode("12345678").unwrap()));

        // Disk Check
        let disk = files[1].file().unwrap();
        assert_eq!(files[1].name(), "mydisk.chd");
        assert_eq!(disk.sha1, Some(hex::decode("9876543210abcdef").unwrap()));
    }
}
