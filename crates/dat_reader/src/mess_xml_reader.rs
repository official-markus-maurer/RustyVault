use crate::dat_store::{DatDir, DatGame, DatHeader, DatNode};
use crate::enums::FileType;
use crate::var_fix;
use roxmltree::{Document, Node};

/// Parser for MESS Software List XML formats.
///
/// `mess_xml_reader.rs` is responsible for parsing the slightly specialized XML format
/// used by MESS (Multi Emulator Super System) software lists. It translates `<software>`,
/// `<dataarea>`, and `<diskarea>` nodes into the standard `DatNode` hierarchy.
///
/// Implementation notes:
/// - Uses `roxmltree` to parse into an immutable in-memory DOM for ergonomic traversal.
pub fn read_mess_xml_dat(xml: &str, filename: &str) -> Result<DatHeader, String> {
    let doc = Document::parse(xml).map_err(|e| format!("Failed to parse XML: {}", e))?;
    let root = doc.root_element();

    let mut dat_header = DatHeader {
        base_dir: DatDir::new(FileType::Dir),
        filename: Some(filename.to_string()),
        ..Default::default()
    };

    if !load_header(&root, &mut dat_header) {
        return Err("Failed to load header".to_string());
    }

    for child in root.children() {
        if !child.is_element() {
            continue;
        }

        if child.tag_name().name() == "software" {
            load_game(&mut dat_header.base_dir, child);
        }
    }

    Ok(dat_header)
}

fn load_header(root: &Node, dat_header: &mut DatHeader) -> bool {
    if root.tag_name().name() != "softwarelist" {
        return false;
    }

    if let Some(name) = root.attribute("name") {
        dat_header.name = Some(name.to_string());
    }
    if let Some(desc) = root.attribute("description") {
        dat_header.description = Some(desc.to_string());
    }

    true
}

fn load_game(parent_dir: &mut DatDir, game_node: Node) {
    let name = match game_node.attribute("name") {
        Some(n) => n.to_string(),
        None => return,
    };

    let mut d_dir = DatNode::new_dir(name, FileType::UnSet);
    let mut d_game = DatGame {
        rom_of: game_node.attribute("romof").map(|s| s.to_string()),
        clone_of: game_node.attribute("cloneof").map(|s| s.to_string()),
        ..Default::default()
    };

    for child in game_node.children() {
        if !child.is_element() {
            continue;
        }

        match child.tag_name().name() {
            "description" => d_game.description = child.text().map(|s| s.to_string()),
            "year" => d_game.year = child.text().map(|s| s.to_string()),
            "publisher" => d_game.manufacturer = child.text().map(|s| s.to_string()),
            "part" => {
                let mut index_continue: Option<usize> = None;
                for part_child in child.children() {
                    if !part_child.is_element() {
                        continue;
                    }

                    match part_child.tag_name().name() {
                        "dataarea" => {
                            for rom_node in part_child.children() {
                                if rom_node.is_element() && rom_node.tag_name().name() == "rom" {
                                    if let Some(d) = d_dir.dir_mut() {
                                        load_rom(d, rom_node, &mut index_continue);
                                    }
                                }
                            }
                        }
                        "diskarea" => {
                            for disk_node in part_child.children() {
                                if disk_node.is_element() && disk_node.tag_name().name() == "disk" {
                                    if let Some(d) = d_dir.dir_mut() {
                                        load_disk(d, disk_node);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(d) = d_dir.dir_mut() {
        d.d_game = Some(Box::new(d_game));
        if !d.children.is_empty() {
            parent_dir.add_child(d_dir.clone());
        }
    }
}

fn load_rom(parent_dir: &mut DatDir, rom_node: Node, index_continue: &mut Option<usize>) {
    let name = rom_node.attribute("name");
    let loadflag = rom_node.attribute("loadflag").unwrap_or("").to_lowercase();

    if let Some(n) = name {
        let mut d_rom = DatNode::new_file(n.to_string(), FileType::UnSet);
        if let Some(f) = d_rom.file_mut() {
            f.size = rom_node.attribute("size").and_then(var_fix::u64_opt);
            f.crc = rom_node
                .attribute("crc")
                .and_then(|s| var_fix::clean_md5_sha1(s, 8));
            f.sha1 = rom_node
                .attribute("sha1")
                .and_then(|s| var_fix::clean_md5_sha1(s, 40));
            f.status = rom_node.attribute("status").map(var_fix::to_lower);
        }
        parent_dir.add_child(d_rom);
        *index_continue = Some(parent_dir.children.len() - 1);
    } else if loadflag == "continue" || loadflag == "ignore" {
        if let Some(idx) = index_continue {
            if let Some(node) = parent_dir.children.get_mut(*idx) {
                if let Some(f) = node.file_mut() {
                    let extra_size = rom_node
                        .attribute("size")
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    if let Some(size) = f.size.as_mut() {
                        *size += extra_size;
                    } else {
                        f.size = Some(extra_size);
                    }
                }
            }
        }
    }
}

fn load_disk(parent_dir: &mut DatDir, disk_node: Node) {
    let name = match disk_node.attribute("name") {
        Some(n) => var_fix::clean_chd(n),
        None => return,
    };

    let mut d_disk = DatNode::new_file(name, FileType::UnSet);
    if let Some(f) = d_disk.file_mut() {
        f.is_disk = true;
        f.sha1 = disk_node
            .attribute("sha1")
            .and_then(|s| var_fix::clean_md5_sha1(s, 40));
        f.status = disk_node.attribute("status").map(var_fix::to_lower);
    }

    parent_dir.add_child(d_disk);
}

#[cfg(test)]
#[path = "tests/mess_xml_reader_tests.rs"]
mod tests;
