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
///   to highly optimize the output structure of the Fix DATs.
/// - The Rust version currently exports the exact structural hierarchy of the missing files without 
///   the advanced flattening passes.
pub struct FixDatReport;

impl FixDatReport {
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
            let has_dat = child.borrow().dat.is_some();
            if is_dir && !has_dat {
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

        // Note: C# calls DatClean.ArchiveDirectoryFlattern and DatClean.RemoveUnNeededDirectories here.
        // We will skip those unless necessary for basic fix DATs.

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

        let mut dat_dir = "Unknown".to_string();
        if dat_root_full_name.len() > 8 {
            let sub = &dat_root_full_name[8..];
            let p = Path::new(sub);
            if let Some(parent) = p.parent() {
                dat_dir = parent.to_string_lossy().replace("\\", "_").replace("/", "_");
            }
        }
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

            let include = (child.dat_status() == DatStatus::InDatCollect || child.dat_status() == DatStatus::InDatMIA) &&
                child.got_status() != GotStatus::Got &&
                (!red_only || !(child.rep_status() == RepStatus::CanBeFixed || child.rep_status() == RepStatus::CanBeFixedMIA || child.rep_status() == RepStatus::CorruptCanBeFixed));

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
                if c.borrow().name == new_parent_name {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recursive_dat_tree_finding_dat() {
        let rv_dat = Rc::new(RefCell::new(RvDat::new()));
        rv_dat.borrow_mut().set_data(DatData::DatName, "TestDat".to_string());

        let t_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut mf = missing_file.borrow_mut();
            mf.name = "missing.rom".to_string();
            mf.dat = Some(Rc::clone(&rv_dat));
            mf.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            mf.set_rep_status(RepStatus::Missing);
        }

        let fixable_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut ff = fixable_file.borrow_mut();
            ff.name = "fixable.rom".to_string();
            ff.dat = Some(Rc::clone(&rv_dat));
            ff.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            ff.set_rep_status(RepStatus::CanBeFixed);
        }

        t_dir.borrow_mut().child_add(Rc::clone(&missing_file));
        t_dir.borrow_mut().child_add(Rc::clone(&fixable_file));

        let out_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        // Test red_only = true (only strictly missing, not fixable)
        let found = FixDatReport::recursive_dat_tree_finding_dat(Rc::clone(&rv_dat), Rc::clone(&t_dir), Rc::clone(&out_dir), true);
        
        assert_eq!(found, 1);
        assert_eq!(out_dir.borrow().children.len(), 1);
        assert_eq!(out_dir.borrow().children[0].borrow().name, "missing.rom");

        // Test red_only = false (all missing/fixable)
        let out_dir_all = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let found_all = FixDatReport::recursive_dat_tree_finding_dat(Rc::clone(&rv_dat), Rc::clone(&t_dir), Rc::clone(&out_dir_all), false);
        
        assert_eq!(found_all, 2);
        assert_eq!(out_dir_all.borrow().children.len(), 2);
    }
}
