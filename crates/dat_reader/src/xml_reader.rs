use crate::dat_store::{DatDir, DatGame, DatHeader, DatNode};
use crate::enums::FileType;
use roxmltree::{Document, Node};

/// Standard XML DAT parser.
/// 
/// `xml_reader.rs` handles standard Logiqx and MAME XML DAT files, converting their
/// `<header>`, `<game>`, `<rom>`, and `<disk>` tags into the internal `DatNode` AST.
/// 
/// Differences from C#:
/// - C# uses `System.Xml.XmlReader` for stateful, forward-only stream parsing to minimize RAM.
/// - The Rust version uses `roxmltree`, which parses the entire XML document into an immutable,
///   in-memory DOM tree in one extremely fast pass, allowing for highly ergonomic node traversal 
///   at the cost of requiring the entire file buffer to fit into RAM simultaneously.
pub fn read_xml_dat(xml: &str, filename: &str) -> Result<DatHeader, String> {
    let doc = Document::parse(xml).map_err(|e| format!("Failed to parse XML: {}", e))?;
    let root = doc.root_element();

    let mut dat_header = DatHeader {
        base_dir: DatDir::new(),
        ..Default::default()
    };

    if !load_header_from_dat(&root, filename, &mut dat_header) {
        return Err("Failed to load header".to_string());
    }

    for child in root.children() {
        if !child.is_element() {
            continue;
        }

        match child.tag_name().name() {
            "dir" => load_dir_from_dat(&mut dat_header.base_dir, child),
            "game" | "machine" => load_game_from_dat(&mut dat_header.base_dir, child),
            "rom" => load_rom_from_dat(&mut dat_header.base_dir, child),
            "disk" => load_disk_from_dat(&mut dat_header.base_dir, child),
            _ => {}
        }
    }

    Ok(dat_header)
}

fn load_header_from_dat(root: &Node, filename: &str, dat_header: &mut DatHeader) -> bool {
    dat_header.filename = Some(filename.to_string());

    let head = match root.children().find(|n| n.has_tag_name("header")) {
        Some(h) => h,
        None => return false,
    };

    for child in head.children() {
        if !child.is_element() {
            continue;
        }
        let text = child.text().map(|s| s.to_string());
        match child.tag_name().name() {
            "id" => dat_header.id = text,
            "name" => dat_header.name = text,
            "type" => dat_header.type_ = text,
            "rootdir" => dat_header.root_dir = text,
            "description" => dat_header.description = text,
            "subset" => dat_header.subset = text,
            "category" => dat_header.category = text,
            "version" => dat_header.version = text,
            "date" => dat_header.date = text,
            "author" => dat_header.author = text,
            "email" => dat_header.email = text,
            "homepage" => dat_header.homepage = text,
            "url" => dat_header.url = text,
            "comment" => dat_header.comment = text,
            "romvault" | "clrmamepro" => {
                if let Some(header) = child.attribute("header") {
                    dat_header.header = Some(header.to_string());
                }
                if let Some(forcepacking) = child.attribute("forcepacking") {
                    dat_header.compression = Some(forcepacking.to_lowercase());
                }
                if let Some(forcemerging) = child.attribute("forcemerging") {
                    dat_header.merge_type = Some(forcemerging.to_lowercase());
                }
                if let Some(dir) = child.attribute("dir") {
                    dat_header.dir = Some(dir.to_lowercase());
                }
            }
            "notzipped" => {
                if let Some(t) = text {
                    let tl = t.to_lowercase();
                    dat_header.not_zipped = tl == "true" || tl == "yes";
                }
            }
            _ => {}
        }
    }

    true
}

fn load_dir_from_dat(parent_dir: &mut DatDir, dir_node: Node) {
    let name = match dir_node.attribute("name") {
        Some(n) => n.to_string(),
        None => return,
    };

    let mut dir = DatNode::new_dir(name, FileType::UnSet);

    for child in dir_node.children() {
        if !child.is_element() {
            continue;
        }
        if let Some(d) = dir.dir_mut() {
            match child.tag_name().name() {
                "dir" => load_dir_from_dat(d, child),
                "game" | "machine" => load_game_from_dat(d, child),
                _ => {}
            }
        }
    }

    parent_dir.add_child(dir);
}

fn load_game_from_dat(parent_dir: &mut DatDir, game_node: Node) {
    let name = match game_node.attribute("name") {
        Some(n) => n.to_string(),
        None => return,
    };

    let file_type = match game_node.attribute("type").map(|s| s.to_lowercase()).as_deref() {
        Some("dir") => FileType::Dir,
        Some("zip") => FileType::Zip,
        Some("7z") => FileType::SevenZip,
        _ => FileType::UnSet,
    };

    let mut d_dir_node = DatNode::new_dir(name, file_type);
    let mut d_game = DatGame::default();

    d_game.id = game_node.attribute("id").map(|s| s.to_string());
    d_game.rom_of = game_node.attribute("romof").map(|s| s.to_string());
    d_game.clone_of = game_node.attribute("cloneof").map(|s| s.to_string());
    d_game.clone_of_id = game_node.attribute("cloneofid").map(|s| s.to_string());
    d_game.sample_of = game_node.attribute("sampleof").map(|s| s.to_string());
    d_game.source_file = game_node.attribute("sourcefile").map(|s| s.to_string());
    d_game.is_bios = game_node.attribute("isbios").map(|s| s.to_string());
    d_game.is_device = game_node.attribute("isdevice").map(|s| s.to_string());
    d_game.board = game_node.attribute("board").map(|s| s.to_string());
    d_game.runnable = game_node.attribute("runnable").map(|s| s.to_string());

    for child in game_node.children() {
        if !child.is_element() {
            continue;
        }

        match child.tag_name().name() {
            "description" => d_game.description = child.text().map(|s| s.to_string()),
            "year" => d_game.year = child.text().map(|s| s.to_string()),
            "manufacturer" => d_game.manufacturer = child.text().map(|s| s.to_string()),
            "tea" | "trurip" | "EmuArc" => {
                d_game.is_emu_arc = true;
                for emu_child in child.children() {
                    if !emu_child.is_element() {
                        continue;
                    }
                    let text = emu_child.text().map(|s| s.to_string());
                    match emu_child.tag_name().name() {
                        "titleid" => d_game.id = text,
                        "publisher" => d_game.publisher = text,
                        "developer" => d_game.developer = text,
                        "year" => d_game.year = text,
                        "genre" => d_game.genre = text,
                        "subgenre" => d_game.sub_genre = text,
                        "ratings" => d_game.ratings = text,
                        "score" => d_game.score = text,
                        "players" => d_game.players = text,
                        "enabled" => d_game.enabled = text,
                        "crc" => d_game.crc = text,
                        "cloneof" => d_game.clone_of = text,
                        "relatedto" => d_game.related_to = text,
                        "source" => d_game.source = text,
                        _ => {}
                    }
                }
            }
            "category" => {
                if let Some(text) = child.text() {
                    d_game.category.push(text.to_string());
                }
            }
            "device_ref" => {
                if let Some(name) = child.attribute("name") {
                    d_game.device_ref.push(name.to_string());
                }
            }
            "rom" => {
                if let Some(d) = d_dir_node.dir_mut() {
                    load_rom_from_dat(d, child);
                }
            }
            "disk" => {
                if let Some(d) = d_dir_node.dir_mut() {
                    load_disk_from_dat(d, child);
                }
            }
            _ => {}
        }
    }

    if let Some(d) = d_dir_node.dir_mut() {
        d.d_game = Some(Box::new(d_game));
    }
    parent_dir.add_child(d_dir_node);
}

fn load_rom_from_dat(parent_dir: &mut DatDir, rom_node: Node) {
    let name = match rom_node.attribute("name") {
        Some(n) => n.to_string(),
        None => return,
    };

    let mut d_file = DatNode::new_file(name, FileType::File);

    if let Some(f) = d_file.file_mut() {
        if let Some(size) = rom_node.attribute("size") {
            f.size = size.parse::<u64>().ok();
        }
        f.crc = rom_node.attribute("crc").and_then(|s| hex::decode(s).ok());
        f.sha1 = rom_node.attribute("sha1").and_then(|s| hex::decode(s).ok());
        f.md5 = rom_node.attribute("md5").and_then(|s| hex::decode(s).ok());
        f.merge = rom_node.attribute("merge").map(|s| s.to_string());
        f.status = rom_node.attribute("status").map(|s| s.to_string());
    }

    parent_dir.add_child(d_file);
}

fn load_disk_from_dat(parent_dir: &mut DatDir, disk_node: Node) {
    let name = match disk_node.attribute("name") {
        Some(n) => format!("{}.chd", n),
        None => return,
    };

    let mut d_file = DatNode::new_file(name, FileType::File);

    if let Some(f) = d_file.file_mut() {
        f.sha1 = disk_node.attribute("sha1").and_then(|s| hex::decode(s).ok());
        f.md5 = disk_node.attribute("md5").and_then(|s| hex::decode(s).ok());
        f.merge = disk_node.attribute("merge").map(|s| s.to_string());
        f.status = disk_node.attribute("status").map(|s| s.to_string());
    }

    parent_dir.add_child(d_file);
}

#[cfg(test)]
#[path = "tests/xml_reader_tests.rs"]
mod tests;
