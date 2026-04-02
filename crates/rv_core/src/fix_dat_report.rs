use std::rc::Rc;
use std::cell::RefCell;
use crate::rv_file::RvFile;
use crate::rv_dat::{RvDat, DatData, DatFlags};
use crate::rv_game::RvGame;
use crate::enums::RepStatus;
use dat_reader::enums::{DatStatus, GotStatus, FileType};
use dat_reader::xml_writer::DatXmlWriter;
use crate::external_dat_converter_to::ExternalDatConverterTo;
use std::path::Path;
use std::fs::File;

/// Engine for exporting "Fix DATs" (lists of missing files).
/// 
/// `FixDatReport` traverses the database to find files marked as `Missing` or `CanBeFixed`
/// and exports a standard XML DAT file containing only those missing files. This allows users
/// to take the Fix DAT to other tools or sites to acquire the missing files.
/// 
/// Differences from C#:
/// - The C# reference calls out to `DatClean.ArchiveDirectoryFlattern` and `DatClean.RemoveUnNeededDirectories`
///   to optimize the output structure of the Fix DATs.
/// - The Rust version now mirrors those cleanup passes with DAT-AST transformations, while still
///   using Rust-native tree traversal and XML serialization infrastructure.
pub struct FixDatReport;

impl FixDatReport {
    fn logical_name_eq(left: &str, right: &str) -> bool {
        #[cfg(windows)]
        {
            left.eq_ignore_ascii_case(right)
        }
        #[cfg(not(windows))]
        {
            left == right
        }
    }

    fn dat_relative_parent_for_output(dat_root_full_name: &str) -> String {
        let dat_root = crate::settings::get_settings().dat_root;
        let dat_root_path = Path::new(if dat_root.is_empty() { "DatRoot" } else { &dat_root });
        let dat_full_path = Path::new(dat_root_full_name);

        crate::settings::strip_physical_prefix(dat_full_path, dat_root_path)
            .and_then(|relative| {
                relative
                    .parent()
                    .map(|parent| parent.to_string_lossy().replace('\\', "_").replace('/', "_"))
            })
            .filter(|parent| !parent.is_empty())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Recursively traverses a directory tree, looking for bound DATs to export as Fix DATs.
    pub fn recursive_dat_tree(out_directory: &str, t_dir_rc: Rc<RefCell<RvFile>>, red_only: bool) {
        let t_dir = t_dir_rc.borrow();

        if t_dir.file_type == FileType::File {
            return;
        }

        if let Some(dat) = t_dir.dat.clone() {
            drop(t_dir);
            Self::extract_dat(out_directory, dat, Rc::clone(&t_dir_rc), red_only);
            return;
        }

        let dir_dats = t_dir.dir_dats.clone();
        if !dir_dats.is_empty() {
            println!("Dats found in {}", t_dir.name);
            for (i, rv_dat) in dir_dats.iter().enumerate() {
                println!("  {} {:?}", i, rv_dat.borrow().get_data(crate::rv_dat::DatData::DatName));
                Self::extract_dat(out_directory, Rc::clone(rv_dat), Rc::clone(&t_dir_rc), red_only);
            }
        }

        let children = t_dir.children.clone();
        drop(t_dir);

        for child in children {
            let is_dir = child.borrow().is_directory();
            if is_dir {
                Self::recursive_dat_tree(out_directory, Rc::clone(&child), red_only);
            }
        }
    }

    /// Extracts the missing components of a single DAT node into an XML file at the target output directory.
    pub fn extract_dat(out_directory: &str, rv_dat_rc: Rc<RefCell<RvDat>>, t_dir_rc: Rc<RefCell<RvFile>>, red_only: bool) {
        let mut out_dir = RvFile::new(FileType::Dir);
        out_dir.dir_dats.push(Rc::clone(&rv_dat_rc));
        let mut out_dir_rc = Rc::new(RefCell::new(out_dir));

        Self::recursive_dat_tree_finding_dat(Rc::clone(&rv_dat_rc), Rc::clone(&t_dir_rc), Rc::clone(&out_dir_rc), red_only);

        let auto_added;
        {
            let dat = rv_dat_rc.borrow();
            auto_added = dat.flag(DatFlags::AUTO_ADDED_DIRECTORY);
        }

        let mut simplify = false;
        {
            let od = out_dir_rc.borrow();
            if auto_added && od.children.len() == 1 {
                let first_child = od.children[0].borrow();
                if first_child.game.is_none() {
                    simplify = true;
                }
            }
        }

        if simplify {
            let first_child = {
                let od = out_dir_rc.borrow();
                Rc::clone(&od.children[0])
            };
            out_dir_rc = first_child;
        }

        if out_dir_rc.borrow().children.is_empty() {
            return;
        }

        Self::fix_single_level_dat(Rc::clone(&out_dir_rc));

        let converter = ExternalDatConverterTo::new();
        let mut dh = match converter.convert_to_external_dat(Rc::clone(&out_dir_rc)) {
            Some(header) => header,
            None => return,
        };

        // Align with C# `DatClean.ArchiveDirectoryFlattern` behavior
        // The Fix DAT export shouldn't contain deeply nested virtual folders unless they are games.
        Self::archive_directory_flatten(&mut dh.base_dir);
        Self::remove_unneeded_directories(&mut dh.base_dir);

        let old_name = dh.name.clone().unwrap_or_default();
        let old_desc = dh.description.clone().unwrap_or_default();
        
        dh.name = Some(format!("FixDat_{}", old_name));
        dh.description = Some(format!("FixDat_{}", old_desc));
        dh.author = Some("RustyRoms".to_string());
        dh.date = Some(chrono::Local::now().format("%Y-%m-%d").to_string());

        let dat_root_full_name = {
            let d = rv_dat_rc.borrow();
            d.get_data(DatData::DatRootFullName).unwrap_or_else(|| "Unknown".to_string())
        };

        let dat_dir = Self::dat_relative_parent_for_output(&dat_root_full_name);
        let p2 = Path::new(&dat_root_full_name);
        let dat_name = p2.file_stem().unwrap_or_default().to_string_lossy().to_string();

        let mut test = 0;
        let mut dat_filename = format!("{}/fixDat_{}_{}.dat", out_directory, dat_dir, dat_name);
        while Path::new(&dat_filename).exists() {
            test += 1;
            dat_filename = format!("{}/fixDat_{}_{}({}).dat", out_directory, dat_dir, dat_name, test);
        }

        if let Ok(mut file) = File::create(&dat_filename) {
            if let Err(e) = DatXmlWriter::write_dat(&mut file, &dh) {
                println!("Failed to write FixDAT: {}", e);
            } else {
                println!("Successfully created FixDAT: {}", dat_filename);
            }
        }
    }

    fn recursive_dat_tree_finding_dat(rv_dat_rc: Rc<RefCell<RvDat>>, t_dir_rc: Rc<RefCell<RvFile>>, out_dir_rc: Rc<RefCell<RvFile>>, red_only: bool) -> i32 {
        let mut found = 0;
        let t_dir = t_dir_rc.borrow();
        let children = t_dir.children.clone();
        drop(t_dir);

        for child_rc in children {
            let child = child_rc.borrow();
            let matches_dat = match &child.dat {
                Some(d) => Rc::ptr_eq(d, &rv_dat_rc),
                None => false,
            };
            
            if !matches_dat {
                continue;
            }

            if child.is_directory() {
                let mut t_copy = RvFile::new(child.file_type);
                t_copy.name = child.name.clone();
                t_copy.game = child.game.clone();
                let t_copy_rc = Rc::new(RefCell::new(t_copy));
                
                drop(child);
                
                let ret = Self::recursive_dat_tree_finding_dat(Rc::clone(&rv_dat_rc), Rc::clone(&child_rc), Rc::clone(&t_copy_rc), red_only);
                found += ret;
                if ret > 0 {
                    out_dir_rc.borrow_mut().child_add(t_copy_rc);
                }
                continue;
            }

            let include = matches!(
                child.dat_status(),
                DatStatus::InDatCollect | DatStatus::InDatMerged | DatStatus::InDatNoDump | DatStatus::InDatMIA
            ) &&
                child.got_status() != GotStatus::Got &&
                (!red_only
                    || !matches!(
                        child.rep_status(),
                        RepStatus::CanBeFixed
                            | RepStatus::CanBeFixedMIA
                            | RepStatus::CorruptCanBeFixed
                            | RepStatus::InToSort
                            | RepStatus::MoveToSort
                            | RepStatus::MoveToCorrupt
                            | RepStatus::Delete
                            | RepStatus::Deleted
                            | RepStatus::NeededForFix
                            | RepStatus::Rename
                            | RepStatus::IncompleteRemove
                    ));

            if include {
                let mut t_copy = RvFile::new(child.file_type);
                t_copy.name = child.name.clone();
                t_copy.size = child.size;
                t_copy.crc = child.crc.clone();
                t_copy.sha1 = child.sha1.clone();
                t_copy.md5 = child.md5.clone();
                t_copy.merge = child.merge.clone();
                t_copy.status = child.status.clone();
                t_copy.set_header_file_type(child.header_file_type());
                t_copy.set_dat_status(child.dat_status());
                t_copy.set_rep_status(child.rep_status());
                
                out_dir_rc.borrow_mut().child_add(Rc::new(RefCell::new(t_copy)));
                found += 1;
            }
        }
        found
    }

    fn fix_single_level_dat(t_dir_rc: Rc<RefCell<RvFile>>) {
        let mut files_to_fix = Vec::new();
        
        {
            let mut t_dir = t_dir_rc.borrow_mut();
            let mut i = 0;
            while i < t_dir.children.len() {
                let child_rc = Rc::clone(&t_dir.children[i]);
                let child = child_rc.borrow();
                
                if child.game.is_some() {
                    i += 1;
                    continue;
                }
                if child.is_directory() {
                    drop(child);
                    Self::fix_single_level_dat(Rc::clone(&t_dir.children[i]));
                    i += 1;
                    continue;
                }
                
                let mut t_copy = RvFile::new(child.file_type);
                t_copy.name = child.name.clone();
                // Copy other necessary properties...
                t_copy.size = child.size;
                t_copy.crc = child.crc.clone();
                t_copy.sha1 = child.sha1.clone();
                t_copy.md5 = child.md5.clone();
                
                files_to_fix.push(Rc::new(RefCell::new(t_copy)));
                t_dir.children.remove(i);
                // do not increment i since we removed
            }
        }
        
        if files_to_fix.is_empty() {
            return;
        }
        
        for file_rc in files_to_fix {
            let mut new_parent_name = file_rc.borrow().name.clone();
            if let Some(pos) = new_parent_name.rfind('.') {
                new_parent_name = new_parent_name[..pos].to_string();
            }
            
            let mut t_dir = t_dir_rc.borrow_mut();
            let mut found_index = None;
            for (idx, c) in t_dir.children.iter().enumerate() {
                if Self::logical_name_eq(&c.borrow().name, &new_parent_name) {
                    found_index = Some(idx);
                    break;
                }
            }
            
            let parent_rc = match found_index {
                Some(idx) => Rc::clone(&t_dir.children[idx]),
                None => {
                    let mut new_parent = RvFile::new(FileType::Dir);
                    new_parent.name = new_parent_name.clone();
                    new_parent.game = Some(Rc::new(RefCell::new(RvGame::from_description(&new_parent_name))));
                    let np_rc = Rc::new(RefCell::new(new_parent));
                    t_dir.child_add(Rc::clone(&np_rc));
                    np_rc
                }
            };
            
            parent_rc.borrow_mut().child_add(file_rc);
        }
    }

    /// Mirrors C# `DatClean.ArchiveDirectoryFlattern`
    /// Flattens sub-directories recursively by prefixing their names to child files,
    /// except when encountering an explicit Game node (which forms the new root).
    fn archive_directory_flatten(d_dir: &mut dat_reader::dat_store::DatDir) {
        if d_dir.d_game.is_some() {
            let mut list = Vec::new();
            Self::archive_flat(d_dir, &mut list, "");
            
            // Clear children and add the flattened list back
            d_dir.children.clear();
            d_dir.children.extend(list);
            return;
        }

        // Keep searching for games down the tree
        for node in &mut d_dir.children {
            if let Some(dat_dir) = node.dir_mut() {
                Self::archive_directory_flatten(dat_dir);
            }
        }
    }

    /// Helper for `archive_directory_flatten`
    fn archive_flat(d_dir: &dat_reader::dat_store::DatDir, new_dir: &mut Vec<dat_reader::dat_store::DatNode>, sub_dir: &str) {
        for node in &d_dir.children {
            let this_name = if sub_dir.is_empty() {
                node.name.to_string()
            } else {
                format!("{}/{}", sub_dir, node.name)
            };

            if let Some(_f) = node.file() {
                let mut new_node = node.clone();
                new_node.name = this_name;
                new_dir.push(new_node);
            } else if let Some(d) = node.dir() {
                let mut new_node = dat_reader::dat_store::DatNode::new_file(format!("{}/", this_name), dat_reader::enums::FileType::UnSet);
                if let Some(f_mut) = new_node.file_mut() {
                    f_mut.size = Some(0);
                    f_mut.crc = Some(vec![0,0,0,0]);
                }
                new_dir.push(new_node);

                Self::archive_flat(d, new_dir, &this_name);
            }
        }
    }

    fn remove_unneeded_directories(d_dir: &mut dat_reader::dat_store::DatDir) {
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
#[path = "tests/fix_dat_report_tests.rs"]
mod tests;
