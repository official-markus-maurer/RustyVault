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
                // dat_header.dir = d_ref.get_data(DatData::DirSetup);
                // dat_header.header = d_ref.get_data(DatData::Header);
                // dat_header.compression = d_ref.get_data(DatData::Compression);
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

        Some(dat_header)
    }

    fn child_add(&self, ext_dir: &mut DatDir, rv_file_rc: Rc<RefCell<RvFile>>) {
        let rv_file = rv_file_rc.borrow();

        if rv_file.file_type == FileType::File {
            match rv_file.rep_status() {
                RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::UnNeeded | 
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
                RepStatus::NotCollected => {
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
}
