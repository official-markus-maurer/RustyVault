use std::rc::Rc;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, trace};
use dat_reader::enums::{DatStatus, FileType, GotStatus};
use crate::enums::RepStatus;
use crate::rv_file::{RvFile, TreeSelect};

/// The logical matching engine for resolving missing ROMs.
/// 
/// `FindFixes` is responsible for calculating the logical repair state (`RepStatus`) of the 
/// database. It identifies missing files in the primary `RustyRoms` and attempts to map them 
/// to available files sitting in `ToSort` using exact CRC/SHA1/MD5 hash matching.
/// 
/// Differences from C#:
/// - The C# reference uses standard Threads to parallelize the creation of `FileGroup` lookup indexes
///   (`FastArraySort.SortWithFilter`).
/// - The Rust version leverages `rayon` to safely build parallel lookup `HashMap` indexes across 
///   available CPU cores, providing equivalent or faster multi-threaded performance while maintaining
///   memory safety without manual thread joining.
pub struct FindFixes;

impl FindFixes {
    fn is_tree_selected(node: &RvFile) -> bool {
        matches!(node.tree_checked, TreeSelect::Selected | TreeSelect::Locked)
    }

    fn source_is_consumable(node: &RvFile) -> bool {
        !matches!(node.tree_checked, TreeSelect::Locked)
            && !matches!(node.dat_status(), DatStatus::InDatCollect | DatStatus::InDatMIA)
    }

    fn got_source_priority(node: &RvFile) -> (u8, u8, u8) {
        let location_priority = match node.dat_status() {
            DatStatus::InDatCollect => 0,
            DatStatus::InDatMIA => 1,
            DatStatus::InDatMerged | DatStatus::InDatNoDump => 2,
            DatStatus::InToSort => 3,
            DatStatus::NotInDat => 4,
        };
        let corruption_priority = match node.got_status() {
            GotStatus::Got => 0,
            GotStatus::Corrupt => 1,
            _ => 2,
        };
        let consumable_priority = if Self::source_is_consumable(node) { 0 } else { 1 };
        (location_priority, corruption_priority, consumable_priority)
    }

    fn preferred_got_idx(
        got_list: &[usize],
        files_got: &[Rc<RefCell<RvFile>>],
        used_got_indices: &HashSet<usize>,
    ) -> Option<usize> {
        got_list
            .iter()
            .copied()
            .filter(|idx| !used_got_indices.contains(idx))
            .min_by_key(|idx| {
                let got = files_got[*idx].borrow();
                let (location_priority, corruption_priority, consumable_priority) =
                    Self::got_source_priority(&got);
                let shared_backing_priority = if Self::has_retained_shared_physical_path(*idx, files_got) {
                    1
                } else {
                    0
                };
                (
                    location_priority,
                    shared_backing_priority,
                    corruption_priority,
                    consumable_priority,
                )
            })
    }

    fn build_physical_path(file: Rc<RefCell<RvFile>>) -> PathBuf {
        let mut path_parts = Vec::new();
        let mut current = Some(file);

        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            let component = node.name_case().to_string();
            if !component.is_empty() {
                path_parts.push(component);
            }
            current = node.parent.as_ref().and_then(|w| w.upgrade());
        }

        path_parts.reverse();
        let logical_path = path_parts.join("\\");
        let has_nested_absolute_component = path_parts
            .iter()
            .skip(1)
            .any(|part| Path::new(part).is_absolute());
        if Path::new(&logical_path).is_absolute() && !has_nested_absolute_component {
            return PathBuf::from(logical_path);
        }
        if let Some(mapped_path) = crate::settings::find_dir_mapping(&logical_path) {
            return PathBuf::from(mapped_path);
        }

        let mut path = PathBuf::new();
        for part in path_parts {
            if Path::new(&part).is_absolute() || path.as_os_str().is_empty() {
                path = PathBuf::from(part);
            } else {
                path.push(part);
            }
        }
        path
    }

    fn build_physical_identity(file: Rc<RefCell<RvFile>>) -> String {
        let mut member_parts = Vec::new();
        let mut current = Some(Rc::clone(&file));

        while let Some(node_rc) = current {
            let (name, parent) = {
                let node = node_rc.borrow();
                (
                    node.name_case().to_string(),
                    node.parent.as_ref().and_then(|w| w.upgrade()),
                )
            };

            let Some(parent_rc) = parent else {
                break;
            };

            let parent_file_type = parent_rc.borrow().file_type;
            if matches!(parent_file_type, FileType::Zip | FileType::SevenZip) {
                if !name.is_empty() {
                    member_parts.push(name);
                }
                member_parts.reverse();
                let archive_path = Self::build_physical_path(parent_rc);
                return format!(
                    "{}::{}",
                    archive_path.to_string_lossy(),
                    member_parts.join("/")
                );
            }

            if !name.is_empty() {
                member_parts.push(name);
            }
            current = Some(parent_rc);
        }

        Self::build_physical_path(file).to_string_lossy().to_string()
    }

    fn physical_identity_eq(left: &str, right: &str) -> bool {
        #[cfg(windows)]
        {
            fn normalize(identity: &str) -> String {
                let (path_part, member_part) = identity.split_once("::").unwrap_or((identity, ""));
                let mut normalized_path = path_part.replace('/', "\\");
                while normalized_path.len() > 3 && normalized_path.ends_with('\\') {
                    normalized_path.pop();
                }
                if member_part.is_empty() {
                    normalized_path
                } else {
                    format!(
                        "{}::{}",
                        normalized_path,
                        member_part.replace('\\', "/").trim_matches('/'),
                    )
                }
            }

            normalize(left).eq_ignore_ascii_case(&normalize(right))
        }
        #[cfg(not(windows))]
        {
            left == right
        }
    }

    fn cleanup_status_retains_shared_path(rep_status: RepStatus) -> bool {
        !matches!(
            rep_status,
            RepStatus::Delete
                | RepStatus::UnNeeded
                | RepStatus::MoveToSort
                | RepStatus::MoveToCorrupt
                | RepStatus::Deleted
        )
    }

    fn dat_status_retains_shared_path(dat_status: DatStatus) -> bool {
        !matches!(dat_status, DatStatus::NotInDat | DatStatus::InToSort)
    }

    fn has_retained_shared_physical_path(
        current_idx: usize,
        files_got: &[Rc<RefCell<RvFile>>],
    ) -> bool {
        let current_path = Self::build_physical_identity(Rc::clone(&files_got[current_idx]));
        files_got.iter().enumerate().any(|(idx, candidate)| {
            if idx == current_idx {
                return false;
            }

            let candidate_ref = candidate.borrow();
            let candidate_path = Self::build_physical_identity(Rc::clone(candidate));
            candidate_ref.got_status() == GotStatus::Got
                && Self::dat_status_retains_shared_path(candidate_ref.dat_status())
                && Self::cleanup_status_retains_shared_path(candidate_ref.rep_status())
                && Self::physical_identity_eq(&candidate_path, &current_path)
        })
    }

    fn merged_cleanup_status(current_idx: usize, files_got: &[Rc<RefCell<RvFile>>]) -> RepStatus {
        if Self::has_retained_shared_physical_path(current_idx, files_got) {
            RepStatus::NotCollected
        } else {
            RepStatus::UnNeeded
        }
    }

    fn extend_unique_got_candidates(
        candidates: &mut Vec<usize>,
        got_list: &[usize],
        seen: &mut HashSet<usize>,
    ) {
        for idx in got_list {
            if seen.insert(*idx) {
                candidates.push(*idx);
            }
        }
    }

    fn has_redundant_romroot_match(
        current: Rc<RefCell<RvFile>>,
        file: &RvFile,
        files_got: &[Rc<RefCell<RvFile>>],
        crc_map: &HashMap<(u64, Vec<u8>), Vec<usize>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Vec<usize>>,
        md5_map: &HashMap<(u64, Vec<u8>), Vec<usize>>,
    ) -> bool {
        let size = file.size.unwrap_or(0);
        let alt_size = file.alt_size.unwrap_or(size);
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();

        if let Some(ref crc) = file.crc {
            if let Some(got_list) = crc_map.get(&(size, crc.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
            if alt_size != size {
                if let Some(got_list) = crc_map.get(&(alt_size, crc.clone())) {
                    Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
                }
            }
        }
        if let Some(ref alt_crc) = file.alt_crc {
            if let Some(got_list) = crc_map.get(&(alt_size, alt_crc.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
        }
        if let Some(ref sha1) = file.sha1 {
            if let Some(got_list) = sha1_map.get(&(size, sha1.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
            if alt_size != size {
                if let Some(got_list) = sha1_map.get(&(alt_size, sha1.clone())) {
                    Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
                }
            }
        }
        if let Some(ref alt_sha1) = file.alt_sha1 {
            if let Some(got_list) = sha1_map.get(&(alt_size, alt_sha1.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
        }
        if let Some(ref md5) = file.md5 {
            if let Some(got_list) = md5_map.get(&(size, md5.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
            if alt_size != size {
                if let Some(got_list) = md5_map.get(&(alt_size, md5.clone())) {
                    Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
                }
            }
        }
        if let Some(ref alt_md5) = file.alt_md5 {
            if let Some(got_list) = md5_map.get(&(alt_size, alt_md5.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
        }

        candidates.into_iter().any(|idx| {
            if Rc::ptr_eq(&files_got[idx], &current) {
                return false;
            }
            let candidate = files_got[idx].borrow();
            candidate.got_status() == GotStatus::Got
                && matches!(
                    candidate.dat_status(),
                    DatStatus::InDatCollect
                        | DatStatus::InDatMIA
                        | DatStatus::InDatMerged
                        | DatStatus::InDatNoDump
                )
        })
    }

    fn has_pending_fix_target_match(
        file: &RvFile,
        files_missing: &[Rc<RefCell<RvFile>>],
        crc_map: &HashMap<(u64, Vec<u8>), Vec<usize>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Vec<usize>>,
        md5_map: &HashMap<(u64, Vec<u8>), Vec<usize>>,
    ) -> bool {
        let size = file.size.unwrap_or(0);
        let alt_size = file.alt_size.unwrap_or(size);
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();

        if let Some(ref crc) = file.crc {
            if let Some(missing_list) = crc_map.get(&(size, crc.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
            if alt_size != size {
                if let Some(missing_list) = crc_map.get(&(alt_size, crc.clone())) {
                    Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
                }
            }
        }
        if let Some(ref alt_crc) = file.alt_crc {
            if let Some(missing_list) = crc_map.get(&(alt_size, alt_crc.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
        }
        if let Some(ref sha1) = file.sha1 {
            if let Some(missing_list) = sha1_map.get(&(size, sha1.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
            if alt_size != size {
                if let Some(missing_list) = sha1_map.get(&(alt_size, sha1.clone())) {
                    Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
                }
            }
        }
        if let Some(ref alt_sha1) = file.alt_sha1 {
            if let Some(missing_list) = sha1_map.get(&(alt_size, alt_sha1.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
        }
        if let Some(ref md5) = file.md5 {
            if let Some(missing_list) = md5_map.get(&(size, md5.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
            if alt_size != size {
                if let Some(missing_list) = md5_map.get(&(alt_size, md5.clone())) {
                    Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
                }
            }
        }
        if let Some(ref alt_md5) = file.alt_md5 {
            if let Some(missing_list) = md5_map.get(&(alt_size, alt_md5.clone())) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
        }

        candidates.into_iter().any(|idx| {
            let candidate = files_missing[idx].borrow();
            matches!(candidate.rep_status(), RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA)
                && matches!(candidate.dat_status(), DatStatus::InDatCollect | DatStatus::InDatMIA)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn hashes_match_file(
        file: &RvFile,
        size: u64,
        alt_size: Option<u64>,
        crc: &Option<Vec<u8>>,
        alt_crc: &Option<Vec<u8>>,
        sha1: &Option<Vec<u8>>,
        alt_sha1: &Option<Vec<u8>>,
        md5: &Option<Vec<u8>>,
        alt_md5: &Option<Vec<u8>>,
    ) -> bool {
        let file_size = file.size.unwrap_or(0);
        let file_alt_size = file.alt_size.unwrap_or(file_size);
        let candidate_alt_size = alt_size.unwrap_or(size);

        let crc_match =
            file.crc.as_ref().zip(crc.as_ref()).is_some_and(|(left, right)| file_size == size && left == right)
            || file.crc.as_ref().zip(alt_crc.as_ref()).is_some_and(|(left, right)| file_size == candidate_alt_size && left == right)
            || file.alt_crc.as_ref().zip(crc.as_ref()).is_some_and(|(left, right)| file_alt_size == size && left == right)
            || file.alt_crc.as_ref().zip(alt_crc.as_ref()).is_some_and(|(left, right)| file_alt_size == candidate_alt_size && left == right);

        let sha1_match =
            file.sha1.as_ref().zip(sha1.as_ref()).is_some_and(|(left, right)| file_size == size && left == right)
            || file.sha1.as_ref().zip(alt_sha1.as_ref()).is_some_and(|(left, right)| file_size == candidate_alt_size && left == right)
            || file.alt_sha1.as_ref().zip(sha1.as_ref()).is_some_and(|(left, right)| file_alt_size == size && left == right)
            || file.alt_sha1.as_ref().zip(alt_sha1.as_ref()).is_some_and(|(left, right)| file_alt_size == candidate_alt_size && left == right);

        let md5_match =
            file.md5.as_ref().zip(md5.as_ref()).is_some_and(|(left, right)| file_size == size && left == right)
            || file.md5.as_ref().zip(alt_md5.as_ref()).is_some_and(|(left, right)| file_size == candidate_alt_size && left == right)
            || file.alt_md5.as_ref().zip(md5.as_ref()).is_some_and(|(left, right)| file_alt_size == size && left == right)
            || file.alt_md5.as_ref().zip(alt_md5.as_ref()).is_some_and(|(left, right)| file_alt_size == candidate_alt_size && left == right);

        crc_match || sha1_match || md5_match
    }

    fn scanned_member_matches_target(file: &RvFile, scanned: &crate::scanned_file::ScannedFile) -> bool {
        Self::hashes_match_file(
            file,
            scanned.size.unwrap_or(0),
            scanned.alt_size,
            &scanned.crc,
            &scanned.alt_crc,
            &scanned.sha1,
            &scanned.alt_sha1,
            &scanned.md5,
            &scanned.alt_md5,
        )
    }

    fn scanned_files_match(
        left: &crate::scanned_file::ScannedFile,
        right: &crate::scanned_file::ScannedFile,
    ) -> bool {
        let left_size = left.size.unwrap_or(0);
        let left_alt_size = left.alt_size.unwrap_or(left_size);
        let right_size = right.size.unwrap_or(0);
        let right_alt_size = right.alt_size.unwrap_or(right_size);

        let crc_match =
            left.crc.as_ref().zip(right.crc.as_ref()).is_some_and(|(a, b)| left_size == right_size && a == b)
            || left.crc.as_ref().zip(right.alt_crc.as_ref()).is_some_and(|(a, b)| left_size == right_alt_size && a == b)
            || left.alt_crc.as_ref().zip(right.crc.as_ref()).is_some_and(|(a, b)| left_alt_size == right_size && a == b)
            || left.alt_crc.as_ref().zip(right.alt_crc.as_ref()).is_some_and(|(a, b)| left_alt_size == right_alt_size && a == b);

        let sha1_match =
            left.sha1.as_ref().zip(right.sha1.as_ref()).is_some_and(|(a, b)| left_size == right_size && a == b)
            || left.sha1.as_ref().zip(right.alt_sha1.as_ref()).is_some_and(|(a, b)| left_size == right_alt_size && a == b)
            || left.alt_sha1.as_ref().zip(right.sha1.as_ref()).is_some_and(|(a, b)| left_alt_size == right_size && a == b)
            || left.alt_sha1.as_ref().zip(right.alt_sha1.as_ref()).is_some_and(|(a, b)| left_alt_size == right_alt_size && a == b);

        let md5_match =
            left.md5.as_ref().zip(right.md5.as_ref()).is_some_and(|(a, b)| left_size == right_size && a == b)
            || left.md5.as_ref().zip(right.alt_md5.as_ref()).is_some_and(|(a, b)| left_size == right_alt_size && a == b)
            || left.alt_md5.as_ref().zip(right.md5.as_ref()).is_some_and(|(a, b)| left_alt_size == right_size && a == b)
            || left.alt_md5.as_ref().zip(right.alt_md5.as_ref()).is_some_and(|(a, b)| left_alt_size == right_alt_size && a == b);

        crc_match || sha1_match || md5_match
    }

    fn node_has_comparable_hashes(node: &RvFile) -> bool {
        node.crc.is_some()
            || node.alt_crc.is_some()
            || node.sha1.is_some()
            || node.alt_sha1.is_some()
            || node.md5.is_some()
            || node.alt_md5.is_some()
    }

    fn scan_physical_node(
        candidate: Rc<RefCell<RvFile>>,
    ) -> Option<crate::scanned_file::ScannedFile> {
        let (candidate_type, candidate_got_status, candidate_name, candidate_has_hashes) = {
            let candidate_ref = candidate.borrow();
            (
                candidate_ref.file_type,
                candidate_ref.got_status(),
                candidate_ref.name_case().to_string(),
                Self::node_has_comparable_hashes(&candidate_ref),
            )
        };

        if candidate_got_status == GotStatus::Got && candidate_has_hashes {
            return None;
        }

        let parent = {
            let candidate_ref = candidate.borrow();
            candidate_ref.parent.as_ref().and_then(|w| w.upgrade())
        };

        match candidate_type {
            FileType::File | FileType::FileOnly => {
                let physical_path = Self::build_physical_path(Rc::clone(&candidate));
                if !physical_path.exists() {
                    return None;
                }
                crate::scanner::Scanner::scan_raw_file(&physical_path.to_string_lossy())
                    .ok()
            }
            FileType::FileZip | FileType::FileSevenZip => {
                let parent = parent?;
                let archive_type = parent.borrow().file_type;
                let archive_path = Self::build_physical_path(Rc::clone(&parent));
                if !archive_path.exists() {
                    return None;
                }
                let time_stamp = fs::metadata(&archive_path)
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|dur| dur.as_secs() as i64)
                    .unwrap_or_default();
                crate::scanner::Scanner::scan_archive_file(archive_type, &archive_path.to_string_lossy(), time_stamp, true)
                    .ok()
                    .and_then(|archive| {
                        archive.children.into_iter().find(|member| {
                            Self::physical_identity_eq(&member.name, &candidate_name)
                        })
                    })
            }
            FileType::Zip | FileType::SevenZip => {
                let archive_path = Self::build_physical_path(Rc::clone(&candidate));
                if !archive_path.exists() {
                    return None;
                }
                let time_stamp = fs::metadata(&archive_path)
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|dur| dur.as_secs() as i64)
                    .unwrap_or_default();
                crate::scanner::Scanner::scan_archive_file(candidate_type, &archive_path.to_string_lossy(), time_stamp, true)
                    .ok()
            }
            _ => None,
        }
    }

    fn read_physical_candidate_matches(
        current: Rc<RefCell<RvFile>>,
        file: &RvFile,
        candidate: Rc<RefCell<RvFile>>,
    ) -> bool {
        let current_scanned = Self::scan_physical_node(current);
        let candidate_scanned = Self::scan_physical_node(candidate);

        if let (Some(current_scanned), Some(candidate_scanned)) = (current_scanned.as_ref(), candidate_scanned.as_ref()) {
            Self::scanned_files_match(current_scanned, candidate_scanned)
        } else if let Some(candidate_scanned) = candidate_scanned.as_ref() {
            Self::scanned_member_matches_target(file, candidate_scanned)
        } else {
            false
        }
    }

    fn has_redundant_physical_dat_match(
        current: Rc<RefCell<RvFile>>,
        file: &RvFile,
        candidate_files: &[Rc<RefCell<RvFile>>],
    ) -> bool {
        candidate_files.iter().any(|candidate| {
            if Rc::ptr_eq(candidate, &current) {
                return false;
            }
            if !matches!(
                candidate.borrow().dat_status(),
                DatStatus::InDatCollect | DatStatus::InDatMIA | DatStatus::InDatMerged | DatStatus::InDatNoDump
            ) {
                return false;
            }
            Self::read_physical_candidate_matches(Rc::clone(&current), file, Rc::clone(candidate))
        })
    }

    fn hydrate_physical_dat_files(candidate_files: &[Rc<RefCell<RvFile>>]) {
        for candidate in candidate_files {
            let needs_refresh = {
                let candidate_ref = candidate.borrow();
                matches!(
                    candidate_ref.dat_status(),
                    DatStatus::InDatCollect | DatStatus::InDatMIA | DatStatus::InDatMerged | DatStatus::InDatNoDump
                ) && (candidate_ref.got_status() != GotStatus::Got || !Self::node_has_comparable_hashes(&candidate_ref))
            };

            if !needs_refresh {
                continue;
            }

            let Some(scanned) = Self::scan_physical_node(Rc::clone(candidate)) else {
                continue;
            };

            let mut candidate_ref = candidate.borrow_mut();
            candidate_ref.got_status = scanned.got_status;
            candidate_ref.file_mod_time_stamp = scanned.file_mod_time_stamp;
            candidate_ref.size = scanned.size;
            candidate_ref.crc = scanned.crc;
            candidate_ref.sha1 = scanned.sha1;
            candidate_ref.md5 = scanned.md5;
            candidate_ref.alt_size = scanned.alt_size;
            candidate_ref.alt_crc = scanned.alt_crc;
            candidate_ref.alt_sha1 = scanned.alt_sha1;
            candidate_ref.alt_md5 = scanned.alt_md5;
            candidate_ref.header_file_type = scanned.header_file_type;
            candidate_ref.local_header_offset = scanned.local_header_offset;
            candidate_ref.cached_stats = None;
        }
    }

    /// Recursively scans the tree to pair `Missing` files with unassigned `Got` files.
    pub fn scan_files(root: Rc<RefCell<RvFile>>) {
        info!("Starting FindFixes pass...");
        // Step 1: Reset tree statuses
        Self::reset_status(Rc::clone(&root));

        // Step 2: Get Selected Files
        let mut all_dat_files = Vec::new();
        Self::get_all_dat_files(Rc::clone(&root), &mut all_dat_files);
        Self::hydrate_physical_dat_files(&all_dat_files);

        let mut files_got = Vec::new();
        let mut files_missing = Vec::new();
        Self::get_selected_files(Rc::clone(&root), &mut files_got, &mut files_missing);
        let mut all_got_files = Vec::new();
        Self::get_all_got_files(Rc::clone(&root), &mut all_got_files);

        info!("FindFixes: Collected {} Got files and {} Missing files.", files_got.len(), files_missing.len());

        // Convert the RC pointers to a form that can be shared across threads
        // We will build the maps using parallel iteration over the files_got list
        let mut hash_data = Vec::with_capacity(files_got.len());
        for (idx, got) in files_got.iter().enumerate() {
            let got_ref = got.borrow();
            hash_data.push((
                idx,
                got_ref.size.unwrap_or(0),
                got_ref.alt_size,
                got_ref.crc.clone(),
                got_ref.alt_crc.clone(),
                got_ref.sha1.clone(),
                got_ref.alt_sha1.clone(),
                got_ref.md5.clone(),
                got_ref.alt_md5.clone()
            ));
        }

        // Now we can use rayon to build the three hash maps in parallel!
        let (crc_map, (sha1_map, md5_map)) = rayon::join(
            || {
                let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                for (idx, size, alt_size, crc, alt_crc, _, _, _, _) in &hash_data {
                    if let Some(c) = crc {
                        map.entry((*size, c.clone())).or_default().push(*idx);
                    }
                    if let Some(c) = alt_crc {
                        map.entry((alt_size.unwrap_or(*size), c.clone())).or_default().push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, sha1, alt_sha1, _, _) in &hash_data {
                            if let Some(s) = sha1 {
                                map.entry((*size, s.clone())).or_default().push(*idx);
                            }
                            if let Some(s) = alt_sha1 {
                                map.entry((alt_size.unwrap_or(*size), s.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, _, _, md5, alt_md5) in &hash_data {
                            if let Some(m) = md5 {
                                map.entry((*size, m.clone())).or_default().push(*idx);
                            }
                            if let Some(m) = alt_md5 {
                                map.entry((alt_size.unwrap_or(*size), m.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    }
                )
            }
        );

        let mut all_got_hash_data = Vec::with_capacity(all_got_files.len());
        for (idx, got) in all_got_files.iter().enumerate() {
            let got_ref = got.borrow();
            all_got_hash_data.push((
                idx,
                got_ref.size.unwrap_or(0),
                got_ref.alt_size,
                got_ref.crc.clone(),
                got_ref.alt_crc.clone(),
                got_ref.sha1.clone(),
                got_ref.alt_sha1.clone(),
                got_ref.md5.clone(),
                got_ref.alt_md5.clone(),
            ));
        }

        let (all_got_crc_map, (all_got_sha1_map, all_got_md5_map)) = rayon::join(
            || {
                let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                for (idx, size, alt_size, crc, alt_crc, _, _, _, _) in &all_got_hash_data {
                    if let Some(c) = crc {
                        map.entry((*size, c.clone())).or_default().push(*idx);
                    }
                    if let Some(c) = alt_crc {
                        map.entry((alt_size.unwrap_or(*size), c.clone())).or_default().push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, sha1, alt_sha1, _, _) in &all_got_hash_data {
                            if let Some(s) = sha1 {
                                map.entry((*size, s.clone())).or_default().push(*idx);
                            }
                            if let Some(s) = alt_sha1 {
                                map.entry((alt_size.unwrap_or(*size), s.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, _, _, md5, alt_md5) in &all_got_hash_data {
                            if let Some(m) = md5 {
                                map.entry((*size, m.clone())).or_default().push(*idx);
                            }
                            if let Some(m) = alt_md5 {
                                map.entry((alt_size.unwrap_or(*size), m.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                )
            },
        );

        let mut missing_hash_data = Vec::with_capacity(files_missing.len());
        for (idx, missing) in files_missing.iter().enumerate() {
            let missing_ref = missing.borrow();
            missing_hash_data.push((
                idx,
                missing_ref.size.unwrap_or(0),
                missing_ref.alt_size,
                missing_ref.crc.clone(),
                missing_ref.alt_crc.clone(),
                missing_ref.sha1.clone(),
                missing_ref.alt_sha1.clone(),
                missing_ref.md5.clone(),
                missing_ref.alt_md5.clone(),
            ));
        }

        let (missing_crc_map, (missing_sha1_map, missing_md5_map)) = rayon::join(
            || {
                let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                for (idx, size, alt_size, crc, alt_crc, _, _, _, _) in &missing_hash_data {
                    if let Some(c) = crc {
                        map.entry((*size, c.clone())).or_default().push(*idx);
                    }
                    if let Some(c) = alt_crc {
                        map.entry((alt_size.unwrap_or(*size), c.clone())).or_default().push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, sha1, alt_sha1, _, _) in &missing_hash_data {
                            if let Some(s) = sha1 {
                                map.entry((*size, s.clone())).or_default().push(*idx);
                            }
                            if let Some(s) = alt_sha1 {
                                map.entry((alt_size.unwrap_or(*size), s.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, _, _, md5, alt_md5) in &missing_hash_data {
                            if let Some(m) = md5 {
                                map.entry((*size, m.clone())).or_default().push(*idx);
                            }
                            if let Some(m) = alt_md5 {
                                map.entry((alt_size.unwrap_or(*size), m.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                )
            },
        );

        let mut used_got_indices = HashSet::new();

        // Step 4: Match Missing files against Got indexes
        for missing in &files_missing {
            let mut missing_ref = missing.borrow_mut();
            let size = missing_ref.size.unwrap_or(0);

            if matches!(missing_ref.dat_status(), DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                missing_ref.set_rep_status(RepStatus::NotCollected);
                missing_ref.cached_stats = None;
                continue;
            }

            let mut found_got_idx = None;
            let mut crc_candidates = Vec::new();
            let mut crc_seen = HashSet::new();

            // Try to find a match by CRC first
            if let Some(ref crc) = missing_ref.crc {
                if let Some(got_list) = crc_map.get(&(size, crc.clone())) {
                    Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                }
            }
            if let Some(ref crc) = missing_ref.crc {
                let alt_size = missing_ref.alt_size.unwrap_or(size);
                if alt_size != size {
                    if let Some(got_list) = crc_map.get(&(alt_size, crc.clone())) {
                        Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                    }
                }
            }
            if let Some(ref alt_crc) = missing_ref.alt_crc {
                let alt_size = missing_ref.alt_size.unwrap_or(size);
                if let Some(got_list) = crc_map.get(&(alt_size, alt_crc.clone())) {
                    Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                }
            }
            if !crc_candidates.is_empty() {
                found_got_idx = Self::preferred_got_idx(&crc_candidates, &files_got, &used_got_indices);
            }

            // Fallback to SHA1 if no CRC match or missing CRC
            if found_got_idx.is_none() {
                let mut sha1_candidates = Vec::new();
                let mut sha1_seen = HashSet::new();
                if let Some(ref sha1) = missing_ref.sha1 {
                    if let Some(got_list) = sha1_map.get(&(size, sha1.clone())) {
                        Self::extend_unique_got_candidates(&mut sha1_candidates, got_list, &mut sha1_seen);
                    }
                }
                if let Some(ref sha1) = missing_ref.sha1 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if alt_size != size {
                        if let Some(got_list) = sha1_map.get(&(alt_size, sha1.clone())) {
                            Self::extend_unique_got_candidates(&mut sha1_candidates, got_list, &mut sha1_seen);
                        }
                    }
                }
                if let Some(ref alt_sha1) = missing_ref.alt_sha1 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if let Some(got_list) = sha1_map.get(&(alt_size, alt_sha1.clone())) {
                        Self::extend_unique_got_candidates(&mut sha1_candidates, got_list, &mut sha1_seen);
                    }
                }
                if !sha1_candidates.is_empty() {
                    found_got_idx = Self::preferred_got_idx(&sha1_candidates, &files_got, &used_got_indices);
                }
            }

            // Fallback to MD5 if still no match
            if found_got_idx.is_none() {
                let mut md5_candidates = Vec::new();
                let mut md5_seen = HashSet::new();
                if let Some(ref md5) = missing_ref.md5 {
                    if let Some(got_list) = md5_map.get(&(size, md5.clone())) {
                        Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                    }
                }
                if let Some(ref md5) = missing_ref.md5 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if alt_size != size {
                        if let Some(got_list) = md5_map.get(&(alt_size, md5.clone())) {
                            Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                        }
                    }
                }
                if let Some(ref alt_md5) = missing_ref.alt_md5 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if let Some(got_list) = md5_map.get(&(alt_size, alt_md5.clone())) {
                        Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                    }
                }
                if !md5_candidates.is_empty() {
                    found_got_idx = Self::preferred_got_idx(&md5_candidates, &files_got, &used_got_indices);
                }
            }

            // If we found a matching file, flag it as fixable
            let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
            if let Some(got_idx) = found_got_idx {
                let got = &files_got[got_idx];
                let is_corrupt = got.borrow().got_status() == GotStatus::Corrupt;
                
                trace!("Found fix match: missing '{}' mapped to got file.", missing_ref.name);
                
                missing_ref.set_rep_status(
                    if is_corrupt {
                        RepStatus::CorruptCanBeFixed
                    } else if is_mia {
                        RepStatus::CanBeFixedMIA
                    } else {
                        RepStatus::CanBeFixed
                    }
                );

                // Mark the got file so it knows it is needed
                let mut got_mut = got.borrow_mut();
                let current_rep = got_mut.rep_status();
                if got_mut.got_status() != GotStatus::Corrupt
                    && (current_rep == RepStatus::UnScanned
                        || current_rep == RepStatus::InToSort
                        || current_rep == RepStatus::MoveToSort
                        || current_rep == RepStatus::Unknown
                        || current_rep == RepStatus::Deleted
                        || current_rep == RepStatus::UnNeeded)
                {
                    got_mut.set_rep_status(RepStatus::NeededForFix);
                }
                if Self::source_is_consumable(&got_mut) {
                    used_got_indices.insert(got_idx);
                }
                
                missing_ref.cached_stats = None;
                got_mut.cached_stats = None;
            } else {
                trace!("No fix found for missing file: {}", missing_ref.name);
                let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
                missing_ref.set_rep_status(
                    if is_mia {
                        RepStatus::MissingMIA
                    } else {
                        RepStatus::Missing
                    }
                );
                missing_ref.cached_stats = None;
            }
        }
        
        // Step 5: Handle corrupt files that aren't needed
        for (idx, got) in files_got.iter().enumerate() {
            let (got_status, rep_status, dat_status) = {
                let got_ref = got.borrow();
                (got_ref.got_status(), got_ref.rep_status(), got_ref.dat_status())
            };
            if got_status == GotStatus::Corrupt {
                let merged_cleanup_status = if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                    Some(Self::merged_cleanup_status(idx, &files_got))
                } else {
                    None
                };
                let mut got_ref = got.borrow_mut();
                if rep_status == RepStatus::NeededForFix {
                    // It's a corrupt file but it matches a needed hash (maybe header issue)
                    // Let's leave it as NeededForFix or CorruptCanBeFixed
                } else if dat_status == DatStatus::InDatCollect {
                    got_ref.set_rep_status(RepStatus::MoveToCorrupt);
                    got_ref.cached_stats = None;
                } else if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                    got_ref.set_rep_status(merged_cleanup_status.unwrap());
                    got_ref.cached_stats = None;
                } else {
                    got_ref.set_rep_status(RepStatus::Delete);
                    got_ref.cached_stats = None;
                }
            }
        }

        // Step 6: Mark remaining unused got files
        for (idx, got) in files_got.iter().enumerate() {
            let (current_rep_status, dat_status) = {
                let got_ref = got.borrow();
                (got_ref.rep_status(), got_ref.dat_status())
            };

            if current_rep_status == RepStatus::NeededForFix || current_rep_status == RepStatus::Correct || current_rep_status == RepStatus::Delete || current_rep_status == RepStatus::MoveToCorrupt {
                continue;
            }

            let merged_cleanup_status = if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                Some(Self::merged_cleanup_status(idx, &files_got))
            } else {
                None
            };

            let should_delete_tosort = if dat_status == DatStatus::InToSort {
                let got_ref = got.borrow();
                Self::has_redundant_romroot_match(
                    Rc::clone(got),
                    &got_ref,
                    &all_got_files,
                    &all_got_crc_map,
                    &all_got_sha1_map,
                    &all_got_md5_map,
                )
                    || Self::has_redundant_physical_dat_match(Rc::clone(got), &got_ref, &all_dat_files)
                    || Self::has_pending_fix_target_match(
                        &got_ref,
                        &files_missing,
                        &missing_crc_map,
                        &missing_sha1_map,
                        &missing_md5_map,
                    )
            } else {
                false
            };

            let mut got_ref = got.borrow_mut();

            if dat_status == DatStatus::InDatCollect {
                got_ref.set_rep_status(RepStatus::Correct);
                got_ref.cached_stats = None;
            } else if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                got_ref.set_rep_status(merged_cleanup_status.unwrap());
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::InDatMIA {
                got_ref.set_rep_status(RepStatus::CorrectMIA);
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::InToSort {
                if should_delete_tosort {
                    got_ref.set_rep_status(RepStatus::Delete);
                } else {
                    got_ref.set_rep_status(RepStatus::InToSort);
                }
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::NotInDat {
                got_ref.set_rep_status(RepStatus::MoveToSort);
                got_ref.cached_stats = None;
            }
        }

        crate::clean_partial::apply_complete_only(Rc::clone(&root));

        // Parity follow-up pass:
        // In the C# reference, after marking partial sets (Incomplete/IncompleteRemove),
        // the engine re-checks the affected FileGroups to settle statuses.
        // Emulate this by running a second condensed pass without the initial reset,
        // rebuilding indexes and re-applying Steps 4-6.
        Self::apply_without_reset(Rc::clone(&root));
    }

    fn reset_status(node: Rc<RefCell<RvFile>>) {
        crate::repair_status::RepairStatus::report_status_reset(node);
    }

    fn get_selected_files(
        node: Rc<RefCell<RvFile>>,
        got_files: &mut Vec<Rc<RefCell<RvFile>>>,
        missing_files: &mut Vec<Rc<RefCell<RvFile>>>,
    ) {
        let n = node.borrow();
        let selected = Self::is_tree_selected(&n);
        
        if selected && !n.is_directory() {
            match n.got_status() {
                GotStatus::Got | GotStatus::Corrupt => {
                    got_files.push(Rc::clone(&node));
                }
                GotStatus::NotGot => {
                    if !matches!(n.dat_status(), DatStatus::NotInDat | DatStatus::InToSort) {
                        missing_files.push(Rc::clone(&node));
                    }
                }
                _ => {}
            }
        }

        if !n.is_directory() {
            return;
        }

        let children = n.children.clone();
        drop(n); // Drop borrow for recursion

        for child in children {
            Self::get_selected_files(child, got_files, missing_files);
        }
    }

    fn get_all_got_files(node: Rc<RefCell<RvFile>>, got_files: &mut Vec<Rc<RefCell<RvFile>>>) {
        let n = node.borrow();

        if !n.is_directory() && matches!(n.got_status(), GotStatus::Got | GotStatus::Corrupt) {
            got_files.push(Rc::clone(&node));
        }

        if !n.is_directory() {
            return;
        }

        let children = n.children.clone();
        drop(n);

        for child in children {
            Self::get_all_got_files(child, got_files);
        }
    }

    fn get_all_dat_files(node: Rc<RefCell<RvFile>>, dat_files: &mut Vec<Rc<RefCell<RvFile>>>) {
        let n = node.borrow();

        if !n.is_directory()
            && matches!(
                n.dat_status(),
                DatStatus::InDatCollect | DatStatus::InDatMIA | DatStatus::InDatMerged | DatStatus::InDatNoDump
            )
        {
            dat_files.push(Rc::clone(&node));
        }

        if !n.is_directory() {
            return;
        }

        let children = n.children.clone();
        drop(n);

        for child in children {
            Self::get_all_dat_files(child, dat_files);
        }
    }

    fn apply_without_reset(root: Rc<RefCell<RvFile>>) {
        let mut all_dat_files = Vec::new();
        Self::get_all_dat_files(Rc::clone(&root), &mut all_dat_files);
        Self::hydrate_physical_dat_files(&all_dat_files);

        let mut files_got = Vec::new();
        let mut files_missing = Vec::new();
        Self::get_selected_files(Rc::clone(&root), &mut files_got, &mut files_missing);
        let mut all_got_files = Vec::new();
        Self::get_all_got_files(Rc::clone(&root), &mut all_got_files);

        let mut hash_data = Vec::with_capacity(files_got.len());
        for (idx, got) in files_got.iter().enumerate() {
            let got_ref = got.borrow();
            hash_data.push((
                idx,
                got_ref.size.unwrap_or(0),
                got_ref.alt_size,
                got_ref.crc.clone(),
                got_ref.alt_crc.clone(),
                got_ref.sha1.clone(),
                got_ref.alt_sha1.clone(),
                got_ref.md5.clone(),
                got_ref.alt_md5.clone(),
            ));
        }

        let (crc_map, (sha1_map, md5_map)) = rayon::join(
            || {
                let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                for (idx, size, alt_size, crc, alt_crc, _, _, _, _) in &hash_data {
                    if let Some(c) = crc {
                        map.entry((*size, c.clone())).or_default().push(*idx);
                    }
                    if let Some(c) = alt_crc {
                        map.entry((alt_size.unwrap_or(*size), c.clone())).or_default().push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, sha1, alt_sha1, _, _) in &hash_data {
                            if let Some(s) = sha1 {
                                map.entry((*size, s.clone())).or_default().push(*idx);
                            }
                            if let Some(s) = alt_sha1 {
                                map.entry((alt_size.unwrap_or(*size), s.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, _, _, md5, alt_md5) in &hash_data {
                            if let Some(m) = md5 {
                                map.entry((*size, m.clone())).or_default().push(*idx);
                            }
                            if let Some(m) = alt_md5 {
                                map.entry((alt_size.unwrap_or(*size), m.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                )
            },
        );

        let mut all_got_hash_data = Vec::with_capacity(all_got_files.len());
        for (idx, got) in all_got_files.iter().enumerate() {
            let got_ref = got.borrow();
            all_got_hash_data.push((
                idx,
                got_ref.size.unwrap_or(0),
                got_ref.alt_size,
                got_ref.crc.clone(),
                got_ref.alt_crc.clone(),
                got_ref.sha1.clone(),
                got_ref.alt_sha1.clone(),
                got_ref.md5.clone(),
                got_ref.alt_md5.clone(),
            ));
        }

        let (all_got_crc_map, (all_got_sha1_map, all_got_md5_map)) = rayon::join(
            || {
                let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                for (idx, size, alt_size, crc, alt_crc, _, _, _, _) in &all_got_hash_data {
                    if let Some(c) = crc {
                        map.entry((*size, c.clone())).or_default().push(*idx);
                    }
                    if let Some(c) = alt_crc {
                        map.entry((alt_size.unwrap_or(*size), c.clone())).or_default().push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, sha1, alt_sha1, _, _) in &all_got_hash_data {
                            if let Some(s) = sha1 {
                                map.entry((*size, s.clone())).or_default().push(*idx);
                            }
                            if let Some(s) = alt_sha1 {
                                map.entry((alt_size.unwrap_or(*size), s.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, _, _, md5, alt_md5) in &all_got_hash_data {
                            if let Some(m) = md5 {
                                map.entry((*size, m.clone())).or_default().push(*idx);
                            }
                            if let Some(m) = alt_md5 {
                                map.entry((alt_size.unwrap_or(*size), m.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                )
            },
        );

        let mut missing_hash_data = Vec::with_capacity(files_missing.len());
        for (idx, missing) in files_missing.iter().enumerate() {
            let missing_ref = missing.borrow();
            missing_hash_data.push((
                idx,
                missing_ref.size.unwrap_or(0),
                missing_ref.alt_size,
                missing_ref.crc.clone(),
                missing_ref.alt_crc.clone(),
                missing_ref.sha1.clone(),
                missing_ref.alt_sha1.clone(),
                missing_ref.md5.clone(),
                missing_ref.alt_md5.clone(),
            ));
        }

        let (missing_crc_map, (missing_sha1_map, missing_md5_map)) = rayon::join(
            || {
                let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                for (idx, size, alt_size, crc, alt_crc, _, _, _, _) in &missing_hash_data {
                    if let Some(c) = crc {
                        map.entry((*size, c.clone())).or_default().push(*idx);
                    }
                    if let Some(c) = alt_crc {
                        map.entry((alt_size.unwrap_or(*size), c.clone())).or_default().push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, sha1, alt_sha1, _, _) in &missing_hash_data {
                            if let Some(s) = sha1 {
                                map.entry((*size, s.clone())).or_default().push(*idx);
                            }
                            if let Some(s) = alt_sha1 {
                                map.entry((alt_size.unwrap_or(*size), s.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, _, _, md5, alt_md5) in &missing_hash_data {
                            if let Some(m) = md5 {
                                map.entry((*size, m.clone())).or_default().push(*idx);
                            }
                            if let Some(m) = alt_md5 {
                                map.entry((alt_size.unwrap_or(*size), m.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                )
            },
        );

        let mut used_got_indices = HashSet::new();

        // Re-apply the core matching and cleanup logic (Steps 4-6)
        for missing in &files_missing {
            let mut missing_ref = missing.borrow_mut();
            let size = missing_ref.size.unwrap_or(0);
            if matches!(missing_ref.dat_status(), DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                missing_ref.set_rep_status(RepStatus::NotCollected);
                missing_ref.cached_stats = None;
                continue;
            }

            let mut found_got_idx = None;
            let mut crc_candidates = Vec::new();
            let mut crc_seen = HashSet::new();
            if let Some(ref crc) = missing_ref.crc {
                if let Some(got_list) = crc_map.get(&(size, crc.clone())) {
                    Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                }
            }
            if let Some(ref crc) = missing_ref.crc {
                let alt_size = missing_ref.alt_size.unwrap_or(size);
                if alt_size != size {
                    if let Some(got_list) = crc_map.get(&(alt_size, crc.clone())) {
                        Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                    }
                }
            }
            if let Some(ref alt_crc) = missing_ref.alt_crc {
                let alt_size = missing_ref.alt_size.unwrap_or(size);
                if let Some(got_list) = crc_map.get(&(alt_size, alt_crc.clone())) {
                    Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                }
            }
            if !crc_candidates.is_empty() {
                found_got_idx = Self::preferred_got_idx(&crc_candidates, &files_got, &used_got_indices);
            }

            if found_got_idx.is_none() {
                let mut sha1_candidates = Vec::new();
                let mut sha1_seen = HashSet::new();
                if let Some(ref sha1) = missing_ref.sha1 {
                    if let Some(got_list) = sha1_map.get(&(size, sha1.clone())) {
                        Self::extend_unique_got_candidates(&mut sha1_candidates, got_list, &mut sha1_seen);
                    }
                }
                if let Some(ref sha1) = missing_ref.sha1 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if alt_size != size {
                        if let Some(got_list) = sha1_map.get(&(alt_size, sha1.clone())) {
                            Self::extend_unique_got_candidates(&mut sha1_candidates, got_list, &mut sha1_seen);
                        }
                    }
                }
                if let Some(ref alt_sha1) = missing_ref.alt_sha1 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if let Some(got_list) = sha1_map.get(&(alt_size, alt_sha1.clone())) {
                        Self::extend_unique_got_candidates(&mut sha1_candidates, got_list, &mut sha1_seen);
                    }
                }
                if !sha1_candidates.is_empty() {
                    found_got_idx = Self::preferred_got_idx(&sha1_candidates, &files_got, &used_got_indices);
                }
            }

            if found_got_idx.is_none() {
                let mut md5_candidates = Vec::new();
                let mut md5_seen = HashSet::new();
                if let Some(ref md5) = missing_ref.md5 {
                    if let Some(got_list) = md5_map.get(&(size, md5.clone())) {
                        Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                    }
                }
                if let Some(ref md5) = missing_ref.md5 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if alt_size != size {
                        if let Some(got_list) = md5_map.get(&(alt_size, md5.clone())) {
                            Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                        }
                    }
                }
                if let Some(ref alt_md5) = missing_ref.alt_md5 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if let Some(got_list) = md5_map.get(&(alt_size, alt_md5.clone())) {
                        Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                    }
                }
                if !md5_candidates.is_empty() {
                    found_got_idx = Self::preferred_got_idx(&md5_candidates, &files_got, &used_got_indices);
                }
            }

            let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
            if let Some(got_idx) = found_got_idx {
                let got = &files_got[got_idx];
                let is_corrupt = got.borrow().got_status() == GotStatus::Corrupt;
                missing_ref.set_rep_status(if is_corrupt {
                    RepStatus::CorruptCanBeFixed
                } else if is_mia {
                    RepStatus::CanBeFixedMIA
                } else {
                    RepStatus::CanBeFixed
                });
                let mut got_mut = got.borrow_mut();
                let current_rep = got_mut.rep_status();
                if got_mut.got_status() != GotStatus::Corrupt
                    && (current_rep == RepStatus::UnScanned
                        || current_rep == RepStatus::InToSort
                        || current_rep == RepStatus::MoveToSort
                        || current_rep == RepStatus::Unknown
                        || current_rep == RepStatus::Deleted
                        || current_rep == RepStatus::UnNeeded)
                {
                    got_mut.set_rep_status(RepStatus::NeededForFix);
                }
                if Self::source_is_consumable(&got_mut) {
                    used_got_indices.insert(got_idx);
                }
                missing_ref.cached_stats = None;
                got_mut.cached_stats = None;
            } else {
                let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
                missing_ref.set_rep_status(if is_mia {
                    RepStatus::MissingMIA
                } else {
                    RepStatus::Missing
                });
                missing_ref.cached_stats = None;
            }
        }

        for (idx, got) in files_got.iter().enumerate() {
            let (got_status, rep_status, dat_status) = {
                let got_ref = got.borrow();
                (got_ref.got_status(), got_ref.rep_status(), got_ref.dat_status())
            };
            if got_status == GotStatus::Corrupt {
                let merged_cleanup_status =
                    if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                        Some(Self::merged_cleanup_status(idx, &files_got))
                    } else {
                        None
                    };
                let mut got_ref = got.borrow_mut();
                if rep_status == RepStatus::NeededForFix {
                } else if dat_status == DatStatus::InDatCollect {
                    got_ref.set_rep_status(RepStatus::MoveToCorrupt);
                    got_ref.cached_stats = None;
                } else if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                    got_ref.set_rep_status(merged_cleanup_status.unwrap());
                    got_ref.cached_stats = None;
                } else {
                    got_ref.set_rep_status(RepStatus::Delete);
                    got_ref.cached_stats = None;
                }
            }
        }

        for (idx, got) in files_got.iter().enumerate() {
            let (current_rep_status, dat_status) = {
                let got_ref = got.borrow();
                (got_ref.rep_status(), got_ref.dat_status())
            };
            if current_rep_status == RepStatus::NeededForFix
                || current_rep_status == RepStatus::Correct
                || current_rep_status == RepStatus::Delete
                || current_rep_status == RepStatus::MoveToCorrupt
            {
                continue;
            }
            let merged_cleanup_status =
                if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                    Some(Self::merged_cleanup_status(idx, &files_got))
                } else {
                    None
                };
            let should_delete_tosort = if dat_status == DatStatus::InToSort {
                let got_ref = got.borrow();
                Self::has_redundant_romroot_match(
                    Rc::clone(got),
                    &got_ref,
                    &all_got_files,
                    &all_got_crc_map,
                    &all_got_sha1_map,
                    &all_got_md5_map,
                ) || Self::has_redundant_physical_dat_match(Rc::clone(got), &got_ref, &all_dat_files)
                    || Self::has_pending_fix_target_match(
                        &got_ref,
                        &files_missing,
                        &missing_crc_map,
                        &missing_sha1_map,
                        &missing_md5_map,
                    )
            } else {
                false
            };
            let mut got_ref = got.borrow_mut();
            if dat_status == DatStatus::InDatCollect {
                got_ref.set_rep_status(RepStatus::Correct);
                got_ref.cached_stats = None;
            } else if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                got_ref.set_rep_status(merged_cleanup_status.unwrap());
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::InDatMIA {
                got_ref.set_rep_status(RepStatus::CorrectMIA);
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::InToSort {
                if should_delete_tosort {
                    got_ref.set_rep_status(RepStatus::Delete);
                } else {
                    got_ref.set_rep_status(RepStatus::InToSort);
                }
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::NotInDat {
                got_ref.set_rep_status(RepStatus::MoveToSort);
                got_ref.cached_stats = None;
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/find_fixes_tests.rs"]
mod tests;
