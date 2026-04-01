use std::rc::Rc;
use std::cell::RefCell;
use crate::rv_file::RvFile;
use crate::rv_dat::DatData;
use crate::rv_game::GameData;
use dat_reader::dat_store::{DatHeader, DatDir, DatNode, DatGame};
use dat_reader::enums::{FileType, HeaderFileType, DatStatus};
use crate::enums::RepStatus;

/// Logic for translating the internal DB tree back into external standard DAT structures.
/// 
/// `ExternalDatConverterTo` recursively walks a given `RvFile` tree branch and generates
/// a `dat_reader::DatHeader` AST representation of it. This is used by the UI's "Export DAT" 
/// functionality, as well as the underlying `FixDatReport` system.
/// 
/// Differences from C#:
/// - The C# implementation contains highly complex flattening rules (`DatClean.ArchiveDirectoryFlattern`)
///   to strip empty folders from the exported DAT.
/// - The Rust version is a more literal 1:1 translation, directly mapping the internal `RvFile` 
///   children to external `DatDir` and `DatGame` nodes based on the applied boolean filters 
///   (`filter_got`, `filter_missing`, etc).
pub struct ExternalDatConverterTo {
    /// Include the XML header block.
    pub use_header: bool,
    /// Include files currently marked as Got.
    pub filter_got: bool,
    /// Include files currently marked as Missing.
    pub filter_missing: bool,
    /// Include files currently marked as CanBeFixed.
    pub filter_fixable: bool,
    /// Include files currently marked as Missing in Action.
    pub filter_mia: bool,
    /// Exclude files marked as Merged.
    pub filter_merged: bool,
    /// Only include loose files (not inside archives).
    pub filter_files: bool,
    /// Only include archive files.
    pub filter_zips: bool,
}

impl ExternalDatConverterTo {
    /// Instantiates a new external converter configured with default boolean inclusion filters.
    pub fn new() -> Self {
        Self {
            use_header: true,
            filter_got: true,
            filter_missing: true,
            filter_fixable: true,
            filter_mia: true,
            filter_merged: false,
            filter_files: true,
            filter_zips: true,
        }
    }

    /// Converts an internal `RvFile` tree branch into an external `DatHeader` AST representation
    /// according to the configured state filters.
    pub fn convert_to_external_dat(&self, rv_file_rc: Rc<RefCell<RvFile>>) -> Option<DatHeader> {
        let rv_file = rv_file_rc.borrow();
        if rv_file.file_type == FileType::File {
            return None;
        }

        let mut dat = None;
        if rv_file.dir_dats.len() == 1 {
            dat = Some(Rc::clone(&rv_file.dir_dats[0]));
        }
        if let Some(ref file_dat) = rv_file.dat {
            dat = Some(Rc::clone(file_dat));
        }

        let mut dat_header = DatHeader::default();

        if let Some(d) = dat {
            if self.use_header {
                let d_ref = d.borrow();
                dat_header.id = d_ref.get_data(DatData::Id);
                dat_header.name = d_ref.get_data(DatData::DatName);
                dat_header.root_dir = d_ref.get_data(DatData::RootDir);
                dat_header.description = d_ref.get_data(DatData::Description);
                dat_header.category = d_ref.get_data(DatData::Category);
                dat_header.version = d_ref.get_data(DatData::Version);
                dat_header.date = d_ref.get_data(DatData::Date);
                dat_header.author = d_ref.get_data(DatData::Author);
                dat_header.email = d_ref.get_data(DatData::Email);
                dat_header.homepage = d_ref.get_data(DatData::HomePage);
                dat_header.url = d_ref.get_data(DatData::Url);
                dat_header.dir = d_ref.get_data(DatData::DirSetup);
                dat_header.header = d_ref.get_data(DatData::Header);
                dat_header.compression = d_ref.get_data(DatData::Compression);
                dat_header.merge_type = d_ref.get_data(DatData::MergeType);
            } else {
                dat_header.name = Some(rv_file.name.clone());
            }
        } else {
            dat_header.name = Some(rv_file.name.clone());
        }

        dat_header.base_dir = DatDir::new();

        for child in &rv_file.children {
            self.child_add(&mut dat_header.base_dir, Rc::clone(child));
        }

        Self::archive_directory_flatten(&mut dat_header.base_dir);
        Self::remove_unneeded_directories(&mut dat_header.base_dir);

        Some(dat_header)
    }

    fn is_archive_type(file_type: FileType) -> bool {
        matches!(file_type, FileType::Zip | FileType::SevenZip)
    }

    fn is_within_archive(rv_file: &RvFile) -> bool {
        let mut current = rv_file.parent.as_ref().and_then(|parent| parent.upgrade());
        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            if Self::is_archive_type(node.file_type) {
                return true;
            }
            current = node.parent.as_ref().and_then(|parent| parent.upgrade());
        }
        false
    }

    fn child_add(&self, ext_dir: &mut DatDir, rv_file_rc: Rc<RefCell<RvFile>>) {
        let rv_file = rv_file_rc.borrow();

        if rv_file.file_type == FileType::File {
            let is_archive_member = Self::is_within_archive(&rv_file);
            if is_archive_member && !self.filter_zips {
                return;
            }
            if !is_archive_member && !self.filter_files {
                return;
            }

            match rv_file.rep_status() {
                RepStatus::Correct | RepStatus::CorrectMIA | 
                RepStatus::Unknown | RepStatus::MoveToSort | RepStatus::Delete | 
                RepStatus::NeededForFix | RepStatus::Rename => {
                    if !self.filter_got { return; }
                },
                RepStatus::Missing | RepStatus::Incomplete => {
                    if !self.filter_missing { return; }
                },
                RepStatus::MissingMIA => {
                    if !self.filter_mia { return; }
                },
                RepStatus::NotCollected | RepStatus::UnNeeded => {
                    if !self.filter_merged { return; }
                },
                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                    if !self.filter_fixable { return; }
                },
                RepStatus::InToSort => {},
                _ => {}
            }

            let mut ext_file = DatNode::new_file(rv_file.name.clone(), FileType::UnSet);
            if let Some(f) = ext_file.file_mut() {
                f.size = rv_file.size;
                f.crc = rv_file.crc.clone();
                f.sha1 = rv_file.sha1.clone();
                f.md5 = rv_file.md5.clone();
                f.merge = Some(rv_file.merge.clone());
                f.status = rv_file.status.clone();

                if rv_file.dat_status() == DatStatus::InDatMIA {
                    f.mia = Some("yes".to_string());
                }

                let is_disk = rv_file.header_file_type() == HeaderFileType::CHD;
                if is_disk {
                    f.is_disk = true;
                    // clean CHD name (strip .chd)
                    if let Some(ref m) = f.merge {
                        if m.to_lowercase().ends_with(".chd") {
                            f.merge = Some(m[..m.len()-4].to_string());
                        }
                    }

                    if rv_file.alt_md5.is_some() || rv_file.alt_sha1.is_some() {
                        f.size = rv_file.alt_size;
                        f.crc = rv_file.alt_crc.clone();
                        f.sha1 = rv_file.alt_sha1.clone();
                        f.md5 = rv_file.alt_md5.clone();
                    }
                }
            }
            
            let is_disk = rv_file.header_file_type() == HeaderFileType::CHD;
            if is_disk && ext_file.name.to_lowercase().ends_with(".chd") {
                ext_file.name = ext_file.name[..ext_file.name.len()-4].to_string();
            }

            ext_dir.add_child(ext_file);
            return;
        }

        if Self::is_archive_type(rv_file.file_type) && !self.filter_zips {
            return;
        }

        let mut game_name = rv_file.name.clone();
        if rv_file.file_type == FileType::Zip && game_name.to_lowercase().ends_with(".zip") {
            game_name = game_name[..game_name.len()-4].to_string();
        } else if rv_file.file_type == FileType::SevenZip && game_name.to_lowercase().ends_with(".7z") {
            game_name = game_name[..game_name.len()-3].to_string();
        }

        let mut ext_dir_1 = DatNode::new_dir(game_name.clone(), FileType::UnSet);

        if let Some(ref g_rc) = rv_file.game {
            let g = g_rc.borrow();
            let mut dat_game = DatGame::default();
            dat_game.description = g.get_data(GameData::Description);
            if let Some(cat) = g.get_data(GameData::Category) {
                dat_game.category = Self::category_list(&cat).unwrap_or_default();
            }
            dat_game.rom_of = g.get_data(GameData::RomOf);
            dat_game.is_bios = g.get_data(GameData::IsBios);
            dat_game.source_file = g.get_data(GameData::Sourcefile);
            dat_game.clone_of = g.get_data(GameData::CloneOf);
            dat_game.sample_of = g.get_data(GameData::SampleOf);
            dat_game.board = g.get_data(GameData::Board);
            dat_game.year = g.get_data(GameData::Year);
            dat_game.manufacturer = g.get_data(GameData::Manufacturer);

            if let Some(desc) = &dat_game.description {
                if desc == "¤" {
                    dat_game.description = Some(game_name.clone());
                }
            }

            if let Some(d) = ext_dir_1.dir_mut() {
                d.d_game = Some(Box::new(dat_game));
            }
        } else if rv_file.file_type == FileType::Zip {
            if let Some(d) = ext_dir_1.dir_mut() {
                d.d_game = Some(Box::new(DatGame::default()));
            }
        }

        for child in &rv_file.children {
            if let Some(d) = ext_dir_1.dir_mut() {
                self.child_add(d, Rc::clone(child));
            }
        }

        ext_dir.add_child(ext_dir_1);
    }

    fn category_list(instr: &str) -> Option<Vec<String>> {
        if instr.trim().is_empty() {
            return None;
        }
        Some(instr.split('|').map(|s| s.trim().to_string()).collect())
    }

    fn archive_directory_flatten(d_dir: &mut DatDir) {
        if d_dir.d_game.is_some() {
            let mut list = Vec::new();
            Self::archive_flat(d_dir, &mut list, "");
            d_dir.children.clear();
            d_dir.children.extend(list);
            return;
        }

        for node in &mut d_dir.children {
            if let Some(dat_dir) = node.dir_mut() {
                Self::archive_directory_flatten(dat_dir);
            }
        }
    }

    fn archive_flat(d_dir: &DatDir, new_dir: &mut Vec<DatNode>, sub_dir: &str) {
        for node in &d_dir.children {
            let this_name = if sub_dir.is_empty() {
                node.name.to_string()
            } else {
                format!("{}/{}", sub_dir, node.name)
            };

            if node.file().is_some() {
                let mut new_node = node.clone();
                new_node.name = this_name;
                new_dir.push(new_node);
            } else if let Some(d) = node.dir() {
                let mut new_node = DatNode::new_file(format!("{}/", this_name), FileType::UnSet);
                if let Some(f_mut) = new_node.file_mut() {
                    f_mut.size = Some(0);
                    f_mut.crc = Some(vec![0, 0, 0, 0]);
                }
                new_dir.push(new_node);

                Self::archive_flat(d, new_dir, &this_name);
            }
        }
    }

    fn remove_unneeded_directories(d_dir: &mut DatDir) {
        let mut kept_children = Vec::new();

        for mut node in d_dir.children.drain(..) {
            let keep = if let Some(child_dir) = node.dir_mut() {
                Self::remove_unneeded_directories(child_dir);
                !child_dir.children.is_empty()
            } else {
                true
            };

            if keep {
                kept_children.push(node);
            }
        }

        d_dir.children = kept_children;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_to_external_dat_flattens_archive_directories() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        zip.borrow_mut().name = "game.zip".to_string();

        let subdir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        subdir.borrow_mut().name = "sub".to_string();

        let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = file.borrow_mut();
            f.name = "inner.bin".to_string();
            f.size = Some(4);
            f.crc = Some(vec![1, 2, 3, 4]);
        }

        subdir.borrow_mut().child_add(Rc::clone(&file));
        zip.borrow_mut().child_add(Rc::clone(&subdir));
        root.borrow_mut().child_add(Rc::clone(&zip));

        let converter = ExternalDatConverterTo::new();
        let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

        assert_eq!(dat.base_dir.children.len(), 1);
        let game_node = dat.base_dir.children[0].dir().unwrap();
        let child_names: Vec<_> = game_node.children.iter().map(|child| child.name.clone()).collect();
        assert_eq!(child_names, vec!["sub/".to_string(), "sub/inner.bin".to_string()]);
        assert!(game_node.children[0].file().is_some());
        assert!(game_node.children[1].file().is_some());
    }

    #[test]
    fn test_convert_to_external_dat_drops_empty_filtered_directories() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut z = zip.borrow_mut();
            z.name = "game.zip".to_string();
        }

        let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = file.borrow_mut();
            f.name = "present.bin".to_string();
            f.set_rep_status(RepStatus::Correct);
        }

        zip.borrow_mut().child_add(Rc::clone(&file));
        root.borrow_mut().child_add(Rc::clone(&zip));

        let mut converter = ExternalDatConverterTo::new();
        converter.filter_got = false;
        let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

        assert!(dat.base_dir.children.is_empty());
    }

    #[test]
    fn test_convert_to_external_dat_can_filter_loose_files_only() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let loose = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = loose.borrow_mut();
            f.name = "loose.bin".to_string();
            f.size = Some(1);
            f.crc = Some(vec![1, 2, 3, 4]);
            f.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&loose));

        let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut z = zip.borrow_mut();
            z.name = "game.zip".to_string();
            z.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&zip));

        let archive_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = archive_file.borrow_mut();
            f.name = "inside.bin".to_string();
            f.size = Some(1);
            f.crc = Some(vec![5, 6, 7, 8]);
            f.parent = Some(Rc::downgrade(&zip));
        }
        zip.borrow_mut().child_add(Rc::clone(&archive_file));

        let mut converter = ExternalDatConverterTo::new();
        converter.filter_zips = false;
        let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

        assert_eq!(dat.base_dir.children.len(), 1);
        assert_eq!(dat.base_dir.children[0].name, "loose.bin");
    }

    #[test]
    fn test_convert_to_external_dat_can_filter_archive_files_only() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let loose = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = loose.borrow_mut();
            f.name = "loose.bin".to_string();
            f.size = Some(1);
            f.crc = Some(vec![1, 2, 3, 4]);
            f.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&loose));

        let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut z = zip.borrow_mut();
            z.name = "game.zip".to_string();
            z.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&zip));

        let archive_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = archive_file.borrow_mut();
            f.name = "inside.bin".to_string();
            f.size = Some(1);
            f.crc = Some(vec![5, 6, 7, 8]);
            f.parent = Some(Rc::downgrade(&zip));
        }
        zip.borrow_mut().child_add(Rc::clone(&archive_file));

        let mut converter = ExternalDatConverterTo::new();
        converter.filter_files = false;
        let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

        assert_eq!(dat.base_dir.children.len(), 1);
        let game = dat.base_dir.children[0].dir().unwrap();
        assert_eq!(dat.base_dir.children[0].name, "game");
        assert_eq!(game.children.len(), 1);
        assert_eq!(game.children[0].name, "inside.bin");
    }

    #[test]
    fn test_convert_to_external_dat_round_trips_extended_header_fields() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let dat = Rc::new(RefCell::new(crate::rv_dat::RvDat::new()));
        {
            let mut d = dat.borrow_mut();
            d.set_data(DatData::Id, Some("id-1".to_string()));
            d.set_data(DatData::DatName, Some("SampleDat".to_string()));
            d.set_data(DatData::RootDir, Some("Roms".to_string()));
            d.set_data(DatData::Header, Some("nes".to_string()));
            d.set_data(DatData::Compression, Some("zip".to_string()));
            d.set_data(DatData::MergeType, Some("split".to_string()));
            d.set_data(DatData::DirSetup, Some("full".to_string()));
        }
        root.borrow_mut().dir_dats.push(Rc::clone(&dat));

        let converter = ExternalDatConverterTo::new();
        let external = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

        assert_eq!(external.id, Some("id-1".to_string()));
        assert_eq!(external.name, Some("SampleDat".to_string()));
        assert_eq!(external.root_dir, Some("Roms".to_string()));
        assert_eq!(external.header, Some("nes".to_string()));
        assert_eq!(external.compression, Some("zip".to_string()));
        assert_eq!(external.merge_type, Some("split".to_string()));
        assert_eq!(external.dir, Some("full".to_string()));
    }

    #[test]
    fn test_convert_to_external_dat_filter_merged_includes_unneeded_entries() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let merged = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = merged.borrow_mut();
            f.name = "merged.bin".to_string();
            f.size = Some(1);
            f.crc = Some(vec![1, 2, 3, 4]);
            f.set_rep_status(RepStatus::UnNeeded);
            f.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&merged));

        let mut converter = ExternalDatConverterTo::new();
        converter.filter_got = false;
        converter.filter_merged = true;
        let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

        assert_eq!(dat.base_dir.children.len(), 1);
        assert_eq!(dat.base_dir.children[0].name, "merged.bin");
    }

    #[test]
    fn test_convert_to_external_dat_filter_got_excludes_unneeded_entries_when_merged_filter_disabled() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let merged = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = merged.borrow_mut();
            f.name = "merged.bin".to_string();
            f.size = Some(1);
            f.crc = Some(vec![1, 2, 3, 4]);
            f.set_rep_status(RepStatus::UnNeeded);
            f.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&merged));

        let mut converter = ExternalDatConverterTo::new();
        converter.filter_got = true;
        converter.filter_merged = false;
        let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

        assert!(dat.base_dir.children.is_empty());
    }
}
