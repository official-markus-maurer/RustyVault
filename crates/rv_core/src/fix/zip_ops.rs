use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use crate::rv_file::RvFile;
use crate::enums::RepStatus;

impl super::Fix {
    pub(super) fn try_zip_move(
        zip_file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) -> bool {
        let mut zip_entries = Vec::new();
        Self::collect_archive_match_entries(Rc::clone(&zip_file), "", &mut zip_entries);
        let mut candidate_archive: Option<Rc<RefCell<RvFile>>> = None;
        let mut has_fixable_child = false;

        for entry in &zip_entries {
            let child_ref = entry.node.borrow();
            if !matches!(
                child_ref.rep_status(),
                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed
            ) {
                continue;
            }

            let Some(source_file) = Self::find_source_file(&child_ref, crc_map, sha1_map, md5_map) else {
                return false;
            };
            let Some((source_archive, _, _)) = Self::find_containing_archive(Rc::clone(&source_file)) else {
                return false;
            };

            let source_archive_type = source_archive.borrow().file_type;
            let target_archive_type = zip_file.borrow().file_type;
            if source_archive_type != target_archive_type {
                return false;
            }

            if let Some(existing_candidate) = candidate_archive.as_ref() {
                if !Rc::ptr_eq(existing_candidate, &source_archive) {
                    return false;
                }
            } else {
                candidate_archive = Some(source_archive);
            }

            has_fixable_child = true;
        }

        if !has_fixable_child {
            return false;
        }

        let Some(source_archive) = candidate_archive else {
            return false;
        };

        let mut source_entries = Vec::new();
        Self::collect_archive_match_entries(Rc::clone(&source_archive), "", &mut source_entries);
        if source_entries.len() != zip_entries.len() {
            return false;
        }
        for target_entry in &zip_entries {
            let target_child_ref = target_entry.node.borrow();
            if !matches!(
                target_child_ref.dat_status(),
                dat_reader::enums::DatStatus::InDatCollect
                    | dat_reader::enums::DatStatus::InDatMerged
                    | dat_reader::enums::DatStatus::InDatNoDump
                    | dat_reader::enums::DatStatus::InDatMIA
            ) {
                continue;
            }

            let found_match = source_entries.iter().any(|source_entry| {
                Self::archive_child_matches_named(
                    &source_entry.node.borrow(),
                    &source_entry.logical_name,
                    &target_child_ref,
                    &target_entry.logical_name,
                )
            });

            if !found_match {
                return false;
            }
        }

        let source_archive_path = Self::get_existing_physical_path(Rc::clone(&source_archive));
        let target_archive_path = Self::get_physical_path(Rc::clone(&zip_file));
        if Self::physical_path_eq_for_rename(Path::new(&source_archive_path), Path::new(&target_archive_path)) {
            return false;
        }

        if let Some(parent) = Path::new(&target_archive_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        if Path::new(&target_archive_path).exists() {
            let _ = std::fs::remove_file(&target_archive_path);
        }

        let source_is_read_only = {
            let source_archive_ref = source_archive.borrow();
            Self::is_fix_read_only(&source_archive_ref)
        };

        let moved_ok = if source_is_read_only {
            std::fs::copy(&source_archive_path, &target_archive_path).is_ok()
        } else {
            std::fs::rename(&source_archive_path, &target_archive_path).is_ok()
                || (std::fs::copy(&source_archive_path, &target_archive_path).is_ok()
                    && std::fs::remove_file(&source_archive_path).is_ok())
        };

        if !moved_ok {
            return false;
        }

        Self::report_action(format!(
            "Move archive: {} -> {}",
            source_archive_path,
            target_archive_path
        ));
        Self::mark_tree_as_got(Rc::clone(&zip_file));
        *total_fixed += 1;

        if !source_is_read_only {
            source_archive.borrow_mut().set_rep_status(RepStatus::Delete);
            queue.push(source_archive);
        }

        true
    }
}
