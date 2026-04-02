use std::rc::Rc;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use flate2::write::DeflateEncoder;
use flate2::Compression;
use libc::{c_char, c_int, c_uint, c_ulong, c_void};
use sevenz_rust::compress_to_path as compress_to_7z_path;
use tracing::{info, debug, trace};
use crate::enums::RepStatus;
use crate::rv_file::{RvFile, TreeSelect};
use dat_reader::enums::{DatStatus, FileType, GotStatus, ZipStructure};
use trrntzip::torrent_zip_check::TorrentZipCheck;
use trrntzip::zipped_file::ZippedFile;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime as ZipDateTime, ZipArchive, ZipWriter};

/// The Fix engine responsible for physically modifying the filesystem.
/// 
/// This module implements the "Fix ROMs" phase of RustyRoms. It traverses the internal file tree
/// and applies physical disk operations (copying, moving, deleting, renaming) to bring the 
/// physical files into alignment with the logical `RepStatus` calculated by `FindFixes`.
/// 
/// Differences from C#:
/// - The C# reference uses a highly abstract `FixAZipCore` virtual I/O engine that can natively 
///   stream and repack `TorrentZip` and `7z` files on the fly.
/// - The Rust implementation currently uses basic `fs::copy`, `fs::rename`, and simple `zip` extraction 
///   without advanced repackaging or `TorrentZip` formatting during the fix pass.
pub struct Fix;

struct StoredZipEntry {
    compressed_data: Vec<u8>,
    crc: u32,
    compressed_size: u32,
    uncompressed_size: u32,
}

struct ArchiveRebuildEntry {
    node: Rc<RefCell<RvFile>>,
    target_name: String,
    existing_name: String,
    is_directory: bool,
}

struct ArchiveMatchEntry {
    node: Rc<RefCell<RvFile>>,
    logical_name: String,
}

struct TorrentZipBuiltEntry {
    name: String,
    compressed_data: Vec<u8>,
    crc: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    flags: u16,
    compression_method: u16,
    external_attributes: u32,
}

type ZAlloc = unsafe extern "C" fn(*mut c_void, c_uint, c_uint) -> *mut c_void;
type ZFree = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
struct ZStream {
    next_in: *mut u8,
    avail_in: c_uint,
    total_in: c_ulong,
    next_out: *mut u8,
    avail_out: c_uint,
    total_out: c_ulong,
    msg: *mut c_char,
    state: *mut c_void,
    zalloc: ZAlloc,
    zfree: ZFree,
    opaque: *mut c_void,
    data_type: c_int,
    adler: c_ulong,
    reserved: c_ulong,
}

#[link(name = "z123", kind = "static")]
unsafe extern "C" {
    fn deflateInit2_(
        strm: *mut ZStream,
        level: c_int,
        method: c_int,
        window_bits: c_int,
        mem_level: c_int,
        strategy: c_int,
        version: *const c_char,
        stream_size: c_int,
    ) -> c_int;
    fn deflate(strm: *mut ZStream, flush: c_int) -> c_int;
    fn deflateEnd(strm: *mut ZStream) -> c_int;
}

impl Fix {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;

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

    fn physical_path_eq_for_rename(left: &Path, right: &Path) -> bool {
        #[cfg(windows)]
        {
            left.to_string_lossy().eq_ignore_ascii_case(&right.to_string_lossy())
        }
        #[cfg(not(windows))]
        {
            left == right
        }
    }

    fn rename_path_if_needed(current_path: &Path, target_path: &Path, temp_suffix: &str) -> std::io::Result<()> {
        if current_path == target_path || !current_path.exists() {
            return Ok(());
        }

        if Self::physical_path_eq_for_rename(current_path, target_path) {
            let mut temp_path = current_path.to_path_buf();
            let temp_name = format!(
                "{}.{}-{}",
                target_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("tmp"),
                temp_suffix,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or_default()
            );
            temp_path.set_file_name(temp_name);
            fs::rename(current_path, &temp_path).and_then(|_| fs::rename(&temp_path, target_path))
        } else {
            fs::rename(current_path, target_path)
        }
    }

    fn torrentzip_datetime() -> Option<ZipDateTime> {
        ZipDateTime::from_date_and_time(1996, 12, 24, 23, 32, 0).ok()
    }

    fn apply_torrentzip_metadata(zip_path: &str) -> bool {
        let Ok(mut zip_bytes) = fs::read(zip_path) else {
            return false;
        };

        let local_header_signature = [0x50, 0x4B, 0x03, 0x04];
        let central_header_signature = [0x50, 0x4B, 0x01, 0x02];
        let utf8_flag = 0x0800u16;

        let mut local_offset = 0usize;
        while local_offset + 30 <= zip_bytes.len()
            && zip_bytes[local_offset..local_offset + 4] == local_header_signature
        {
            let flags = u16::from_le_bytes([
                zip_bytes[local_offset + 6],
                zip_bytes[local_offset + 7],
            ]);
            let normalized_flags = 0x0002 | (flags & utf8_flag);

            zip_bytes[local_offset + 4..local_offset + 6].copy_from_slice(&20u16.to_le_bytes());
            zip_bytes[local_offset + 6..local_offset + 8]
                .copy_from_slice(&normalized_flags.to_le_bytes());
            zip_bytes[local_offset + 8..local_offset + 10].copy_from_slice(&8u16.to_le_bytes());
            zip_bytes[local_offset + 10..local_offset + 12]
                .copy_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            zip_bytes[local_offset + 12..local_offset + 14]
                .copy_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());

            let compressed_size = u32::from_le_bytes([
                zip_bytes[local_offset + 18],
                zip_bytes[local_offset + 19],
                zip_bytes[local_offset + 20],
                zip_bytes[local_offset + 21],
            ]) as usize;
            let file_name_length = u16::from_le_bytes([
                zip_bytes[local_offset + 26],
                zip_bytes[local_offset + 27],
            ]) as usize;
            let extra_length = u16::from_le_bytes([
                zip_bytes[local_offset + 28],
                zip_bytes[local_offset + 29],
            ]) as usize;

            local_offset += 30 + file_name_length + extra_length + compressed_size;
        }

        let eocd_signature = [0x50, 0x4B, 0x05, 0x06];
        let Some(eocd_offset) = zip_bytes
            .windows(4)
            .rposition(|window| window == eocd_signature) else {
            return false;
        };

        if eocd_offset + 22 > zip_bytes.len() {
            return false;
        }

        let central_directory_size = u32::from_le_bytes([
            zip_bytes[eocd_offset + 12],
            zip_bytes[eocd_offset + 13],
            zip_bytes[eocd_offset + 14],
            zip_bytes[eocd_offset + 15],
        ]) as usize;
        let central_directory_offset = u32::from_le_bytes([
            zip_bytes[eocd_offset + 16],
            zip_bytes[eocd_offset + 17],
            zip_bytes[eocd_offset + 18],
            zip_bytes[eocd_offset + 19],
        ]) as usize;

        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return false;
        }

        let central_directory_end = central_directory_offset + central_directory_size;
        let mut central_offset = central_directory_offset;
        while central_offset + 46 <= central_directory_end
            && zip_bytes[central_offset..central_offset + 4] == central_header_signature
        {
            let flags = u16::from_le_bytes([
                zip_bytes[central_offset + 8],
                zip_bytes[central_offset + 9],
            ]);
            let normalized_flags = 0x0002 | (flags & utf8_flag);

            zip_bytes[central_offset + 4..central_offset + 6].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 6..central_offset + 8].copy_from_slice(&20u16.to_le_bytes());
            zip_bytes[central_offset + 8..central_offset + 10]
                .copy_from_slice(&normalized_flags.to_le_bytes());
            zip_bytes[central_offset + 10..central_offset + 12].copy_from_slice(&8u16.to_le_bytes());
            zip_bytes[central_offset + 12..central_offset + 14]
                .copy_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            zip_bytes[central_offset + 14..central_offset + 16]
                .copy_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());
            zip_bytes[central_offset + 34..central_offset + 36].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 36..central_offset + 38].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 38..central_offset + 40].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 40..central_offset + 42].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 42..central_offset + 46].copy_from_slice(&0u32.to_le_bytes());

            let file_name_length = u16::from_le_bytes([
                zip_bytes[central_offset + 28],
                zip_bytes[central_offset + 29],
            ]) as usize;
            let extra_length = u16::from_le_bytes([
                zip_bytes[central_offset + 30],
                zip_bytes[central_offset + 31],
            ]) as usize;
            let comment_length = u16::from_le_bytes([
                zip_bytes[central_offset + 32],
                zip_bytes[central_offset + 33],
            ]) as usize;

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        let mut crc_hasher = crc32fast::Hasher::new();
        crc_hasher.update(
            &zip_bytes[central_directory_offset..central_directory_offset + central_directory_size],
        );
        let comment = format!("TORRENTZIPPED-{:08X}", crc_hasher.finalize());
        let comment_bytes = comment.into_bytes();

        zip_bytes[eocd_offset + 20..eocd_offset + 22]
            .copy_from_slice(&(comment_bytes.len() as u16).to_le_bytes());
        zip_bytes.truncate(eocd_offset + 22);
        zip_bytes.extend_from_slice(&comment_bytes);

        fs::write(zip_path, zip_bytes).is_ok()
    }

    fn is_fix_selected(node: &RvFile) -> bool {
        matches!(node.tree_checked, TreeSelect::Selected | TreeSelect::Locked)
    }

    fn is_fix_read_only(node: &RvFile) -> bool {
        matches!(node.tree_checked, TreeSelect::Locked)
    }

    fn has_selected_descendant(node: Rc<RefCell<RvFile>>) -> bool {
        let children = node.borrow().children.clone();
        for child in children {
            if Self::is_fix_selected(&child.borrow()) || Self::has_selected_descendant(Rc::clone(&child)) {
                return true;
            }
        }
        false
    }

    /// Executes the fix operations across the database tree using a task queue, matching C# logic.
    pub fn perform_fixes(root: Rc<RefCell<RvFile>>) {
        info!("Starting Fix execution pass...");
        let mut file_process_queue = Vec::new();
        let mut total_fixed = 0;

        // In order to not slow down the single-threaded rust fix process with DB searches,
        // we pre-compute the hash map of needed files (this is a Rust optimization 
        // that replaces the need for FindSourceFile.cs deep tree traversals)
        let mut needed_files = Vec::new();
        Self::gather_needed_files(Rc::clone(&root), &mut needed_files);
        
        let mut crc_map: HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>> = HashMap::new();
        let mut sha1_map: HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>> = HashMap::new();
        let mut md5_map: HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>> = HashMap::new();

        for needed in needed_files {
            let n_ref = needed.borrow();
            let size = n_ref.size.unwrap_or(0);
            let alt_size = n_ref.alt_size.unwrap_or(size);
            if let Some(ref crc) = n_ref.crc { crc_map.insert((size, crc.clone()), Rc::clone(&needed)); }
            if let Some(ref crc) = n_ref.alt_crc { crc_map.insert((alt_size, crc.clone()), Rc::clone(&needed)); }
            if let Some(ref sha1) = n_ref.sha1 { sha1_map.insert((size, sha1.clone()), Rc::clone(&needed)); }
            if let Some(ref sha1) = n_ref.alt_sha1 { sha1_map.insert((alt_size, sha1.clone()), Rc::clone(&needed)); }
            if let Some(ref md5) = n_ref.md5 { md5_map.insert((size, md5.clone()), Rc::clone(&needed)); }
            if let Some(ref md5) = n_ref.alt_md5 { md5_map.insert((alt_size, md5.clone()), Rc::clone(&needed)); }
        }

        let children = root.borrow().children.clone();
        for child in children {
            Self::fix_base(Rc::clone(&child), &mut file_process_queue, &mut total_fixed, &crc_map, &sha1_map, &md5_map);
            while !file_process_queue.is_empty() {
                let queued_file = file_process_queue.remove(0);
                Self::fix_base(queued_file, &mut file_process_queue, &mut total_fixed, &crc_map, &sha1_map, &md5_map);
            }
        }
        
        info!("Fix execution complete. Total fixed: {}", total_fixed);
    }

    fn fix_dir(
        dir: Rc<RefCell<RvFile>>, 
        queue: &mut Vec<Rc<RefCell<RvFile>>>, 
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) {
        let children = dir.borrow().children.clone();

        for child in children {
            Self::fix_base(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map);

            // Process the operation queue (simulating C# fileProcessQueue)
            while !queue.is_empty() {
                let queued_file = queue.remove(0);
                Self::fix_base(queued_file, queue, total_fixed, crc_map, sha1_map, md5_map);
            }
        }
    }

    fn fix_base(
        child: Rc<RefCell<RvFile>>, 
        queue: &mut Vec<Rc<RefCell<RvFile>>>, 
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) {
        if child.borrow().rep_status() == RepStatus::Deleted {
            return;
        }

        let (file_type, rep_status, is_selected) = {
            let child_ref = child.borrow();
            (
                child_ref.file_type,
                child_ref.rep_status(),
                Self::is_fix_selected(&child_ref),
            )
        };

        match file_type {
            FileType::Zip | FileType::SevenZip => {
                if matches!(
                    rep_status,
                    RepStatus::Delete | RepStatus::UnNeeded | RepStatus::MoveToSort | RepStatus::MoveToCorrupt | RepStatus::Rename
                ) {
                    Self::fix_archive_node(Rc::clone(&child));
                    return;
                }
                if !is_selected && !Self::has_selected_descendant(Rc::clone(&child)) { return; }
                // In C#: returnCode = FixAZip.FixZip(child, fileProcessQueue, ref totalFixed, out errorMessage);
                // For now, we will process Zip contents directly if they are flagged for fixes.
                Self::fix_a_zip(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map);
            }
            FileType::Dir => {
                if is_selected {
                    let has_name = !child.borrow().name.is_empty();
                    if has_name {
                        Self::rename_directory_if_needed(Rc::clone(&child));
                    }
                }
                Self::fix_dir(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map);
            }
            FileType::File | FileType::FileOnly | FileType::FileSevenZip | FileType::FileZip => {
                if !is_selected { return; }
                Self::fix_a_file(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map);
            }
            _ => {}
        }
    }

    fn gather_needed_files(dir: Rc<RefCell<RvFile>>, needed: &mut Vec<Rc<RefCell<RvFile>>>) {
        let d = dir.borrow();
        for child in &d.children {
            if child.borrow().is_directory() {
                Self::gather_needed_files(Rc::clone(child), needed);
            } else if child.borrow().rep_status() == RepStatus::NeededForFix
                && Self::is_fix_selected(&child.borrow())
            {
                needed.push(Rc::clone(child));
            }
        }
    }

    fn get_physical_path(file: Rc<RefCell<RvFile>>) -> String {
        Self::build_physical_path(file, false).to_string_lossy().replace('\\', "/")
    }

    fn get_existing_physical_path(file: Rc<RefCell<RvFile>>) -> String {
        Self::build_physical_path(file, true).to_string_lossy().replace('\\', "/")
    }

    fn build_physical_path(file: Rc<RefCell<RvFile>>, use_existing_names: bool) -> PathBuf {
        let mut path_parts = Vec::new();
        let mut current = Some(file);
        
        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            let component = if use_existing_names {
                node.name_case()
            } else {
                &node.name
            };
            if !component.is_empty() {
                path_parts.push(component.to_string());
            }
            current = node.parent.as_ref().and_then(|w| w.upgrade());
        }
        
        path_parts.reverse();
        let logical_path = path_parts.join("\\");
        if Path::new(&logical_path).is_absolute() {
            return PathBuf::from(logical_path);
        }
        if let Some(mapped_path) = crate::settings::find_dir_mapping(&logical_path) {
            return PathBuf::from(mapped_path);
        }

        let mut path = PathBuf::new();
        for part in path_parts {
            if path.as_os_str().is_empty() {
                path = PathBuf::from(part);
            } else {
                path.push(part);
            }
        }
        path
    }

    fn find_tree_root(node: Rc<RefCell<RvFile>>) -> Rc<RefCell<RvFile>> {
        let mut current = node;
        loop {
            let parent = {
                let borrowed = current.borrow();
                borrowed.parent.as_ref().and_then(|w| w.upgrade())
            };
            if let Some(parent) = parent {
                current = parent;
            } else {
                return current;
            }
        }
    }

    fn status_retains_shared_physical_path(rep_status: RepStatus) -> bool {
        !matches!(
            rep_status,
            RepStatus::Delete
                | RepStatus::UnNeeded
                | RepStatus::MoveToSort
                | RepStatus::MoveToCorrupt
                | RepStatus::Deleted
        )
    }

    fn dat_status_retains_shared_physical_path(dat_status: DatStatus) -> bool {
        !matches!(dat_status, DatStatus::NotInDat | DatStatus::InToSort)
    }

    fn has_retained_shared_physical_path(
        root: Rc<RefCell<RvFile>>,
        current: Rc<RefCell<RvFile>>,
        current_path: &Path,
    ) -> bool {
        let children = root.borrow().children.clone();
        for child in children {
            if Self::node_retains_shared_physical_path(Rc::clone(&child), Rc::clone(&current), current_path) {
                return true;
            }
        }
        false
    }

    fn node_retains_shared_physical_path(
        node: Rc<RefCell<RvFile>>,
        current: Rc<RefCell<RvFile>>,
        current_path: &Path,
    ) -> bool {
        if Rc::ptr_eq(&node, &current) {
            return false;
        }

        let (is_dir, got_status, dat_status, rep_status, children) = {
            let borrowed = node.borrow();
            (
                borrowed.is_directory(),
                borrowed.got_status(),
                borrowed.dat_status(),
                borrowed.rep_status(),
                borrowed.children.clone(),
            )
        };

        if !is_dir
            && got_status == GotStatus::Got
            && Self::dat_status_retains_shared_physical_path(dat_status)
            && Self::status_retains_shared_physical_path(rep_status)
        {
            let candidate_path = Self::build_physical_path(Rc::clone(&node), true);
            if Self::physical_path_eq_for_rename(&candidate_path, current_path) {
                return true;
            }
        }

        if is_dir {
            for child in children {
                if Self::node_retains_shared_physical_path(child, Rc::clone(&current), current_path) {
                    return true;
                }
            }
        }

        false
    }

    fn rename_directory_if_needed(dir: Rc<RefCell<RvFile>>) {
        let current_path = Self::build_physical_path(Rc::clone(&dir), true);
        let target_path = Self::build_physical_path(Rc::clone(&dir), false);

        let rename_result = Self::rename_path_if_needed(&current_path, &target_path, "tmpdir");

        if rename_result.is_ok() {
            let mut dir_mut = dir.borrow_mut();
            dir_mut.file_name = dir_mut.name.clone();
        }
    }

    fn get_tosort_path(file_path: &str, base_dir: &str) -> String {
        let path = Path::new(file_path);
        let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();

        let mapped_base_dir = base_dir.replace('/', "\\");
        let mapped_base_path =
            crate::settings::find_dir_mapping(&mapped_base_dir).unwrap_or_else(|| mapped_base_dir.clone());
        if let Some((source_logical_key, source_root_path)) =
            crate::settings::find_mapping_for_physical_path(path)
        {
            if let Some(relative_path) = crate::settings::strip_physical_prefix(path, &source_root_path) {
                let mut relative_dirs: Vec<String> = relative_path
                    .parent()
                    .map(|parent| {
                        parent
                            .components()
                            .filter_map(|component| match component {
                                std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                if Self::logical_name_eq(&source_logical_key, "ToSort")
                    && Self::logical_name_eq(&mapped_base_dir, "ToSort\\Corrupt")
                    && relative_dirs
                        .first()
                        .is_some_and(|s| Self::logical_name_eq(s, "Corrupt"))
                {
                    relative_dirs.remove(0);
                }

                let mut dir_path = PathBuf::from(&mapped_base_path);
                for part in &relative_dirs {
                    dir_path.push(part);
                }
                let _ = fs::create_dir_all(&dir_path);

                let mut target_path_buf = dir_path.join(file_name);
                if target_path_buf == path {
                    return target_path_buf.to_string_lossy().replace('\\', "/");
                }

                let mut target_path = target_path_buf.to_string_lossy().replace('\\', "/");
                let mut counter = 0;
                while Path::new(&target_path).exists() {
                    let file_stem = path.file_stem().unwrap().to_str().unwrap();
                    let ext = path.extension().map(|e| e.to_str().unwrap()).unwrap_or("");

                    let new_name = if ext.is_empty() {
                        format!("{}_{}", file_stem, counter)
                    } else {
                        format!("{}_{}.{}", file_stem, counter, ext)
                    };
                    target_path_buf = dir_path.join(new_name);
                    target_path = target_path_buf.to_string_lossy().replace('\\', "/");
                    counter += 1;
                }

                return target_path;
            }
        }

        let mut root_base = PathBuf::new();
        let mut normal_components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    root_base.push(component.as_os_str());
                }
                std::path::Component::Normal(part) => {
                    normal_components.push(part.to_string_lossy().to_string());
                }
                _ => {}
            }
        }

        if let Some(first_normal) = normal_components.first() {
            root_base.push(first_normal);
        }

        let mut relative_dirs = if normal_components.len() > 1 {
            normal_components[1..normal_components.len().saturating_sub(1)].to_vec()
        } else {
            Vec::new()
        };

        let normalized_base_dir = base_dir.replace('/', "\\");
        let base_parts: Vec<String> = normalized_base_dir
            .split('\\')
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect();

        let shares_root = normal_components
            .first()
            .zip(base_parts.first())
            .is_some_and(|(path_root, base_root)| Self::logical_name_eq(path_root, base_root));

        if shares_root {
            let base_suffix: Vec<String> = base_parts.iter().skip(1).cloned().collect();
            let already_prefixed = relative_dirs.len() >= base_suffix.len()
                && base_suffix
                    .iter()
                    .zip(relative_dirs.iter())
                    .all(|(base_part, relative_part)| Self::logical_name_eq(base_part, relative_part));

            if !base_suffix.is_empty() && !already_prefixed {
                let mut prefixed = base_suffix;
                prefixed.extend(relative_dirs);
                relative_dirs = prefixed;
            }
        } else {
            let mut prefixed = base_parts.clone();
            prefixed.extend(relative_dirs);
            relative_dirs = prefixed;
        }

        let mut dir_path = root_base;
        for part in &relative_dirs {
            dir_path.push(part);
        }

        let _ = fs::create_dir_all(&dir_path);

        let mut target_path_buf = dir_path.join(file_name);
        if target_path_buf == path {
            return target_path_buf.to_string_lossy().replace('\\', "/");
        }

        let mut target_path = target_path_buf.to_string_lossy().replace('\\', "/");
        let mut counter = 0;
        while Path::new(&target_path).exists() {
            let file_stem = path.file_stem().unwrap().to_str().unwrap();
            let ext = path.extension().map(|e| e.to_str().unwrap()).unwrap_or("");
            
            let new_name = if ext.is_empty() {
                format!("{}_{}", file_stem, counter)
            } else {
                format!("{}_{}.{}", file_stem, counter, ext)
            };
            target_path_buf = dir_path.join(new_name);
            target_path = target_path_buf.to_string_lossy().replace('\\', "/");
            counter += 1;
        }
        
        target_path
    }

    fn get_archive_member_tosort_path(archive_path: &Path, child_name: &str, base_dir: &str) -> PathBuf {
        let mapped_base_dir = base_dir.replace('/', "\\");
        let mapped_base_path =
            crate::settings::find_dir_mapping(&mapped_base_dir).unwrap_or_else(|| mapped_base_dir.clone());
        if let Some((source_logical_key, source_root_path)) =
            crate::settings::find_mapping_for_physical_path(archive_path)
        {
            if let Some(relative_archive_path) = crate::settings::strip_physical_prefix(archive_path, &source_root_path) {
                let archive_name = relative_archive_path.file_name().unwrap_or_default();
                let mut target_path = PathBuf::from(&mapped_base_path);
                let mut relative_dirs: Vec<String> = relative_archive_path
                    .parent()
                    .map(|parent| {
                        parent
                            .components()
                            .filter_map(|component| match component {
                                std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                if Self::logical_name_eq(&source_logical_key, "ToSort")
                    && Self::logical_name_eq(&mapped_base_dir, "ToSort\\Corrupt")
                    && relative_dirs
                        .first()
                        .is_some_and(|s| Self::logical_name_eq(s, "Corrupt"))
                {
                    relative_dirs.remove(0);
                }

                for part in &relative_dirs {
                    target_path.push(part);
                }
                target_path.push(archive_name);

                for part in child_name.split(['/', '\\']).filter(|part| !part.is_empty()) {
                    target_path.push(part);
                }

                if let Some(parent) = target_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                return target_path;
            }
        }

        let archive_parent = archive_path.parent().unwrap_or_else(|| Path::new(""));
        let archive_name = archive_path.file_name().unwrap_or_default();

        let mut root_base = PathBuf::new();
        let mut normal_components = Vec::new();

        for component in archive_parent.components() {
            match component {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    root_base.push(component.as_os_str());
                }
                std::path::Component::Normal(part) => {
                    normal_components.push(part.to_string_lossy().to_string());
                }
                _ => {}
            }
        }

        if let Some(first_normal) = normal_components.first() {
            root_base.push(first_normal);
        }

        let mut relative_dirs = if normal_components.len() > 1 {
            normal_components[1..].to_vec()
        } else {
            Vec::new()
        };

        let normalized_base_dir = base_dir.replace('/', "\\");
        let base_parts: Vec<String> = normalized_base_dir
            .split('\\')
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect();

        let shares_root = normal_components
            .first()
            .zip(base_parts.first())
            .is_some_and(|(path_root, base_root)| Self::logical_name_eq(path_root, base_root));

        if shares_root {
            let base_suffix: Vec<String> = base_parts.iter().skip(1).cloned().collect();
            let already_prefixed = relative_dirs.len() >= base_suffix.len()
                && base_suffix
                    .iter()
                    .zip(relative_dirs.iter())
                    .all(|(base_part, relative_part)| Self::logical_name_eq(base_part, relative_part));

            if !base_suffix.is_empty() && !already_prefixed {
                let mut prefixed = base_suffix;
                prefixed.extend(relative_dirs);
                relative_dirs = prefixed;
            }
        } else {
            let mut prefixed = base_parts;
            prefixed.extend(relative_dirs);
            relative_dirs = prefixed;
        }

        let mut target_path = root_base;
        for part in &relative_dirs {
            target_path.push(part);
        }
        target_path.push(archive_name);

        for part in child_name.split(['/', '\\']).filter(|part| !part.is_empty()) {
            target_path.push(part);
        }

        if let Some(parent) = target_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        target_path
    }

    fn find_source_file(
        file: &RvFile,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) -> Option<Rc<RefCell<RvFile>>> {
        let size = file.size.unwrap_or(0);
        let alt_size = file.alt_size.unwrap_or(size);

        if let Some(ref crc) = file.crc {
            if let Some(found) = crc_map.get(&(size, crc.clone())) {
                return Some(Rc::clone(found));
            }
        }
        if let Some(ref crc) = file.crc {
            if alt_size != size {
                if let Some(found) = crc_map.get(&(alt_size, crc.clone())) {
                    return Some(Rc::clone(found));
                }
            }
        }
        if let Some(ref alt_crc) = file.alt_crc {
            if let Some(found) = crc_map.get(&(alt_size, alt_crc.clone())) {
                return Some(Rc::clone(found));
            }
        }

        if let Some(ref sha1) = file.sha1 {
            if let Some(found) = sha1_map.get(&(size, sha1.clone())) {
                return Some(Rc::clone(found));
            }
        }
        if let Some(ref sha1) = file.sha1 {
            if alt_size != size {
                if let Some(found) = sha1_map.get(&(alt_size, sha1.clone())) {
                    return Some(Rc::clone(found));
                }
            }
        }
        if let Some(ref alt_sha1) = file.alt_sha1 {
            if let Some(found) = sha1_map.get(&(alt_size, alt_sha1.clone())) {
                return Some(Rc::clone(found));
            }
        }

        if let Some(ref md5) = file.md5 {
            if let Some(found) = md5_map.get(&(size, md5.clone())) {
                return Some(Rc::clone(found));
            }
        }
        if let Some(ref md5) = file.md5 {
            if alt_size != size {
                if let Some(found) = md5_map.get(&(alt_size, md5.clone())) {
                    return Some(Rc::clone(found));
                }
            }
        }
        if let Some(ref alt_md5) = file.alt_md5 {
            if let Some(found) = md5_map.get(&(alt_size, alt_md5.clone())) {
                return Some(Rc::clone(found));
            }
        }

        None
    }

    fn collect_archive_rebuild_entries(
        parent: Rc<RefCell<RvFile>>,
        target_prefix: &str,
        existing_prefix: &str,
        entries: &mut Vec<ArchiveRebuildEntry>,
        any_changes: &mut bool,
    ) {
        let children = parent.borrow().children.clone();
        for child in children {
            let (child_name, existing_child_name, is_directory) = {
                let child_ref = child.borrow();
                let child_name = if target_prefix.is_empty() {
                    child_ref.name.clone()
                } else {
                    format!("{}/{}", target_prefix, child_ref.name)
                };
                let existing_child_name = if existing_prefix.is_empty() {
                    child_ref.name_case().to_string()
                } else {
                    format!("{}/{}", existing_prefix, child_ref.name_case())
                };
                (child_name, existing_child_name, child_ref.is_directory())
            };

            if child_name != existing_child_name {
                *any_changes = true;
            }

            if is_directory {
                let has_children = !child.borrow().children.is_empty();
                if !has_children {
                    entries.push(ArchiveRebuildEntry {
                        node: Rc::clone(&child),
                        target_name: child_name.clone(),
                        existing_name: existing_child_name.clone(),
                        is_directory: true,
                    });
                }
                Self::collect_archive_rebuild_entries(
                    Rc::clone(&child),
                    &child_name,
                    &existing_child_name,
                    entries,
                    any_changes,
                );
            } else {
                entries.push(ArchiveRebuildEntry {
                    node: Rc::clone(&child),
                    target_name: child_name,
                    existing_name: existing_child_name,
                    is_directory: false,
                });
            }
        }
    }

    fn archive_child_matches_named(
        source_child: &RvFile,
        source_name: &str,
        target_child: &RvFile,
        target_name: &str,
    ) -> bool {
        if source_name != target_name {
            return false;
        }
        if source_child.size != target_child.size {
            return false;
        }
        if target_child.crc.is_some() && source_child.crc != target_child.crc {
            return false;
        }
        if target_child.sha1.is_some() && source_child.sha1 != target_child.sha1 {
            return false;
        }
        if target_child.md5.is_some() && source_child.md5 != target_child.md5 {
            return false;
        }

        true
    }

    fn collect_archive_match_entries(
        parent: Rc<RefCell<RvFile>>,
        prefix: &str,
        entries: &mut Vec<ArchiveMatchEntry>,
    ) {
        let children = parent.borrow().children.clone();
        for child in children {
            let (logical_name, is_directory) = {
                let child_ref = child.borrow();
                let logical_name = if prefix.is_empty() {
                    child_ref.name.clone()
                } else {
                    format!("{}/{}", prefix, child_ref.name)
                };
                (logical_name, child_ref.is_directory())
            };

            if is_directory {
                Self::collect_archive_match_entries(Rc::clone(&child), &logical_name, entries);
            } else {
                entries.push(ArchiveMatchEntry {
                    node: Rc::clone(&child),
                    logical_name,
                });
            }
        }
    }

    fn mark_tree_as_got(node: Rc<RefCell<RvFile>>) {
        let children = {
            let mut node_ref = node.borrow_mut();
            let dat_status = node_ref.dat_status();
            node_ref.set_got_status(dat_reader::enums::GotStatus::Got);
            node_ref.set_rep_status(match dat_status {
                dat_reader::enums::DatStatus::InDatMIA => RepStatus::CorrectMIA,
                dat_reader::enums::DatStatus::InToSort => RepStatus::InToSort,
                dat_reader::enums::DatStatus::NotInDat => RepStatus::Unknown,
                _ => RepStatus::Correct,
            });
            node_ref.cached_stats = None;
            node_ref.children.clone()
        };

        for child in children {
            Self::mark_tree_as_got(child);
        }
    }

    fn fix_archive_node(archive: Rc<RefCell<RvFile>>) {
        let (rep_status, current_path, target_path, is_read_only) = {
            let archive_ref = archive.borrow();
            let current_path = Self::build_physical_path(Rc::clone(&archive), true);
            let target_path = Self::build_physical_path(Rc::clone(&archive), false);
            (
                archive_ref.rep_status(),
                current_path,
                target_path,
                Self::is_fix_read_only(&archive_ref),
            )
        };

        if is_read_only {
            return;
        }

        match rep_status {
            RepStatus::Delete | RepStatus::UnNeeded => {
                let root = Self::find_tree_root(Rc::clone(&archive));
                if Self::has_retained_shared_physical_path(root, Rc::clone(&archive), &current_path) {
                    let mut archive_mut = archive.borrow_mut();
                    archive_mut.set_got_status(GotStatus::NotGot);
                    archive_mut.rep_status_reset();
                    return;
                }
                if Path::new(&current_path).exists() {
                    let _ = fs::remove_file(&current_path);
                }
                let mut archive_mut = archive.borrow_mut();
                archive_mut.set_got_status(GotStatus::NotGot);
                archive_mut.rep_status_reset();
            }
            RepStatus::MoveToSort => {
                let current_path_str = current_path.to_string_lossy();
                let tosort_path = Self::get_tosort_path(&current_path_str, "ToSort");
                let tosort_path = PathBuf::from(tosort_path);
                if current_path.exists() {
                    let _ = Self::rename_path_if_needed(&current_path, &tosort_path, "tmptosort");
                }
                let mut archive_mut = archive.borrow_mut();
                archive_mut.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                archive_mut.rep_status_reset();
            }
            RepStatus::MoveToCorrupt => {
                let current_path_str = current_path.to_string_lossy();
                let tosort_path = Self::get_tosort_path(&current_path_str, "ToSort/Corrupt");
                let tosort_path = PathBuf::from(tosort_path);
                if current_path.exists() {
                    let _ = Self::rename_path_if_needed(&current_path, &tosort_path, "tmptosort");
                }
                let mut archive_mut = archive.borrow_mut();
                archive_mut.set_got_status(GotStatus::NotGot);
                archive_mut.rep_status_reset();
            }
            RepStatus::Rename => {
                let _ = Self::rename_path_if_needed(&current_path, &target_path, "tmpfile");
                {
                    let mut archive_mut = archive.borrow_mut();
                    archive_mut.file_name = archive_mut.name.clone();
                    archive_mut.set_got_status(GotStatus::Got);
                    archive_mut.rep_status_reset();
                }
            }
            _ => {}
        }
    }

    fn read_zip_entry_bytes(zip_path: &str, entry_name: &str) -> Option<Vec<u8>> {
        let file = File::open(zip_path).ok()?;
        let mut archive = ZipArchive::new(file).ok()?;
        let mut exact_match = None;
        let mut logical_match = None;

        for index in 0..archive.len() {
            let Ok(entry) = archive.by_index(index) else {
                continue;
            };
            if entry.name() == entry_name {
                exact_match = Some(index);
                break;
            }
            if logical_match.is_none() && Self::logical_name_eq(entry.name(), entry_name) {
                logical_match = Some(index);
            }
        }

        let mut entry = archive.by_index(exact_match.or(logical_match)?).ok()?;
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer).ok()?;
        Some(buffer)
    }

    fn read_raw_zip_entry(zip_path: &str, entry_name: &str) -> Option<StoredZipEntry> {
        let zip_bytes = fs::read(zip_path).ok()?;
        let eocd_offset = zip_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])?;

        if eocd_offset + 22 > zip_bytes.len() {
            return None;
        }

        let central_directory_size = u32::from_le_bytes([
            zip_bytes[eocd_offset + 12],
            zip_bytes[eocd_offset + 13],
            zip_bytes[eocd_offset + 14],
            zip_bytes[eocd_offset + 15],
        ]) as usize;
        let central_directory_offset = u32::from_le_bytes([
            zip_bytes[eocd_offset + 16],
            zip_bytes[eocd_offset + 17],
            zip_bytes[eocd_offset + 18],
            zip_bytes[eocd_offset + 19],
        ]) as usize;

        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return None;
        }

        let mut central_offset = central_directory_offset;
        while central_offset + 46 <= central_directory_offset + central_directory_size {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return None;
            }

            let compression_method = u16::from_le_bytes([
                zip_bytes[central_offset + 10],
                zip_bytes[central_offset + 11],
            ]);
            let crc = u32::from_le_bytes([
                zip_bytes[central_offset + 16],
                zip_bytes[central_offset + 17],
                zip_bytes[central_offset + 18],
                zip_bytes[central_offset + 19],
            ]);
            let compressed_size = u32::from_le_bytes([
                zip_bytes[central_offset + 20],
                zip_bytes[central_offset + 21],
                zip_bytes[central_offset + 22],
                zip_bytes[central_offset + 23],
            ]);
            let uncompressed_size = u32::from_le_bytes([
                zip_bytes[central_offset + 24],
                zip_bytes[central_offset + 25],
                zip_bytes[central_offset + 26],
                zip_bytes[central_offset + 27],
            ]);
            let file_name_length = u16::from_le_bytes([
                zip_bytes[central_offset + 28],
                zip_bytes[central_offset + 29],
            ]) as usize;
            let extra_length = u16::from_le_bytes([
                zip_bytes[central_offset + 30],
                zip_bytes[central_offset + 31],
            ]) as usize;
            let comment_length = u16::from_le_bytes([
                zip_bytes[central_offset + 32],
                zip_bytes[central_offset + 33],
            ]) as usize;
            let relative_offset = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]) as usize;

            let name_start = central_offset + 46;
            let name_end = name_start + file_name_length;
            if name_end > zip_bytes.len() {
                return None;
            }

            let current_name = String::from_utf8_lossy(&zip_bytes[name_start..name_end]);
            if Self::logical_name_eq(&current_name, entry_name) {
                if compression_method != 8 {
                    return None;
                }

                if relative_offset + 30 > zip_bytes.len()
                    || zip_bytes[relative_offset..relative_offset + 4] != [0x50, 0x4B, 0x03, 0x04]
                {
                    return None;
                }

                let local_name_length = u16::from_le_bytes([
                    zip_bytes[relative_offset + 26],
                    zip_bytes[relative_offset + 27],
                ]) as usize;
                let local_extra_length = u16::from_le_bytes([
                    zip_bytes[relative_offset + 28],
                    zip_bytes[relative_offset + 29],
                ]) as usize;
                let data_offset = relative_offset + 30 + local_name_length + local_extra_length;
                let data_end = data_offset + compressed_size as usize;

                if data_end > zip_bytes.len() {
                    return None;
                }

                return Some(StoredZipEntry {
                    compressed_data: zip_bytes[data_offset..data_end].to_vec(),
                    crc,
                    compressed_size,
                    uncompressed_size,
                });
            }

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        None
    }

    fn find_containing_archive(
        file: Rc<RefCell<RvFile>>,
    ) -> Option<(Rc<RefCell<RvFile>>, String, FileType)> {
        let mut path_parts = Vec::new();
        let mut current = Some(file);

        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            let component = node.name_case().to_string();
            let parent = node.parent.as_ref().and_then(|p| p.upgrade());
            drop(node);

            let Some(parent_rc) = parent else {
                return None;
            };

            let parent_type = parent_rc.borrow().file_type;
            if matches!(parent_type, FileType::Zip | FileType::SevenZip) {
                if !component.is_empty() {
                    path_parts.push(component);
                }
                path_parts.reverse();
                return Some((parent_rc, path_parts.join("/"), parent_type));
            }

            if !component.is_empty() {
                path_parts.push(component);
            }
            current = Some(parent_rc);
        }

        None
    }

    fn make_temp_extract_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), unique))
    }

    fn read_seven_zip_entry_bytes(archive_path: &str, entry_name: &str) -> Option<Vec<u8>> {
        let temp_dir = Self::make_temp_extract_dir("rvfix_7z_extract");
        let _ = fs::create_dir_all(&temp_dir);
        let mut buffer = Vec::new();
        let mut found = false;
        let status = sevenz_rust::decompress_file_with_extract_fn(
            archive_path,
            &temp_dir,
            |entry, reader, _dest| {
                if Self::logical_name_eq(entry.name(), entry_name) {
                    found = true;
                    std::io::copy(reader, &mut buffer)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
        );
        let _ = fs::remove_dir_all(&temp_dir);
        if status.is_ok() && found {
            Some(buffer)
        } else {
            None
        }
    }

    fn read_source_file_bytes(source_file: Rc<RefCell<RvFile>>) -> Option<Vec<u8>> {
        let source_path = Self::get_existing_physical_path(Rc::clone(&source_file));

        if let Some((parent_archive, source_name, parent_type)) = Self::find_containing_archive(Rc::clone(&source_file)) {
            let archive_path = Self::get_existing_physical_path(parent_archive);
            return match parent_type {
                FileType::Zip => Self::read_zip_entry_bytes(&archive_path, &source_name),
                FileType::SevenZip => Self::read_seven_zip_entry_bytes(&archive_path, &source_name),
                _ => fs::read(&source_path).ok(),
            };
        }

        fs::read(&source_path).ok()
    }

    fn queue_source_cleanup(source_file: Rc<RefCell<RvFile>>, queue: &mut Vec<Rc<RefCell<RvFile>>>) {
        let parent_archive = Self::find_containing_archive(Rc::clone(&source_file)).map(|(archive, _, _)| archive);

        if let Some(parent_archive) = parent_archive {
            source_file.borrow_mut().set_rep_status(RepStatus::Delete);
            if !queue.iter().any(|queued| Rc::ptr_eq(queued, &parent_archive)) {
                queue.push(parent_archive);
            }
        } else {
            source_file.borrow_mut().set_rep_status(RepStatus::Delete);
            queue.push(source_file);
        }
    }

    fn source_uses_same_archive_path(source_file: Rc<RefCell<RvFile>>, target_archive_path: &Path) -> bool {
        Self::find_containing_archive(source_file)
            .map(|(archive, _, _)| {
                let source_archive_path = Self::get_existing_physical_path(archive);
                Self::physical_path_eq_for_rename(Path::new(&source_archive_path), target_archive_path)
            })
            .unwrap_or(false)
    }

    fn torrentzip_flags(name: &str) -> u16 {
        0x0002 | if name.is_ascii() { 0 } else { 0x0800 }
    }

    fn compress_torrentzip_entry(name: &str, entry_bytes: &[u8]) -> Option<TorrentZipBuiltEntry> {
        let compressed_data = Self::deflate_with_native_zlib(entry_bytes)
            .or_else(|| {
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
                encoder.write_all(entry_bytes).ok()?;
                encoder.finish().ok()
            })?;

        let mut crc_hasher = crc32fast::Hasher::new();
        crc_hasher.update(entry_bytes);
        let crc = crc_hasher.finalize();

        Some(TorrentZipBuiltEntry {
            name: name.to_string(),
            compressed_size: compressed_data.len() as u32,
            uncompressed_size: entry_bytes.len() as u32,
            compressed_data,
            crc,
            flags: Self::torrentzip_flags(name),
            compression_method: 8,
            external_attributes: 0,
        })
    }

    fn deflate_with_native_zlib(entry_bytes: &[u8]) -> Option<Vec<u8>> {
        const Z_OK: c_int = 0;
        const Z_FINISH: c_int = 4;
        const Z_STREAM_END: c_int = 1;
        const Z_DEFLATED: c_int = 8;
        const Z_DEFAULT_STRATEGY: c_int = 0;
        const Z_BEST_COMPRESSION: c_int = 9;
        const ZLIB_123_VERSION: &[u8] = b"1.2.3\0";

        unsafe extern "C" fn zlib_alloc(
            _opaque: *mut c_void,
            items: c_uint,
            size: c_uint,
        ) -> *mut c_void {
            libc::malloc(items as usize * size as usize)
        }

        unsafe extern "C" fn zlib_free(_opaque: *mut c_void, address: *mut c_void) {
            libc::free(address);
        }

        unsafe {
            let mut stream = ZStream {
                next_in: entry_bytes.as_ptr() as *mut u8,
                avail_in: entry_bytes.len().try_into().ok()?,
                total_in: 0,
                next_out: std::ptr::null_mut(),
                avail_out: 0,
                total_out: 0,
                msg: std::ptr::null_mut(),
                state: std::ptr::null_mut(),
                zalloc: zlib_alloc,
                zfree: zlib_free,
                opaque: std::ptr::null_mut(),
                data_type: 0,
                adler: 0,
                reserved: 0,
            };

            let init_result = deflateInit2_(
                &mut stream,
                Z_BEST_COMPRESSION,
                Z_DEFLATED,
                -15,
                8,
                Z_DEFAULT_STRATEGY,
                ZLIB_123_VERSION.as_ptr() as *const c_char,
                std::mem::size_of::<ZStream>() as c_int,
            );

            if init_result != Z_OK {
                return None;
            }

            let mut output = vec![0u8; entry_bytes.len().saturating_add(entry_bytes.len() / 10).saturating_add(64)];
            let mut success = false;

            loop {
                if stream.total_out as usize == output.len() {
                    output.resize(output.len().saturating_mul(2).max(64), 0);
                }

                stream.next_out = output[stream.total_out as usize..].as_mut_ptr();
                stream.avail_out = (output.len() - stream.total_out as usize).try_into().ok()?;

                let result = deflate(&mut stream, Z_FINISH);
                if result == Z_STREAM_END {
                    success = true;
                    break;
                }
                if result != Z_OK {
                    break;
                }
            }

            let _ = deflateEnd(&mut stream);
            if !success {
                return None;
            }

            output.truncate(stream.total_out as usize);
            Some(output)
        }
    }

    fn maybe_reuse_torrentzip_stream_from_source(
        source_file: Rc<RefCell<RvFile>>,
    ) -> Option<TorrentZipBuiltEntry> {
        let (parent_archive, entry_name, parent_type) = Self::find_containing_archive(source_file)?;
        if parent_type != FileType::Zip {
            return None;
        }

        let archive_path = Self::get_existing_physical_path(Rc::clone(&parent_archive));
        let stored = Self::read_raw_zip_entry(&archive_path, &entry_name)?;

        Some(TorrentZipBuiltEntry {
            name: entry_name.clone(),
            compressed_data: stored.compressed_data,
            crc: stored.crc,
            compressed_size: stored.compressed_size,
            uncompressed_size: stored.uncompressed_size,
            flags: Self::torrentzip_flags(&entry_name),
            compression_method: 8,
            external_attributes: 0,
        })
    }

    fn maybe_reuse_torrentzip_stream_from_existing(
        zip_path: &str,
        entry_name: &str,
    ) -> Option<TorrentZipBuiltEntry> {
        let stored = Self::read_raw_zip_entry(zip_path, entry_name)?;
        Some(TorrentZipBuiltEntry {
            name: entry_name.to_string(),
            compressed_data: stored.compressed_data,
            crc: stored.crc,
            compressed_size: stored.compressed_size,
            uncompressed_size: stored.uncompressed_size,
            flags: Self::torrentzip_flags(entry_name),
            compression_method: 8,
            external_attributes: 0,
        })
    }

    fn build_torrentzip_directory_entry(name: &str) -> TorrentZipBuiltEntry {
        let entry_name = if name.ends_with('/') {
            name.to_string()
        } else {
            format!("{}/", name)
        };
        TorrentZipBuiltEntry {
            name: entry_name.clone(),
            compressed_data: Vec::new(),
            crc: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            flags: Self::torrentzip_flags(&entry_name),
            compression_method: 0,
            external_attributes: 0x10,
        }
    }

    fn build_torrentzip_archive(entries: &[TorrentZipBuiltEntry]) -> Option<Vec<u8>> {
        let mut archive_bytes = Vec::new();
        let mut central_directory = Vec::new();

        for entry in entries {
            let name_bytes = entry.name.as_bytes();
            let local_offset = archive_bytes.len() as u32;

            archive_bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
            archive_bytes.extend_from_slice(&20u16.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.flags.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.compression_method.to_le_bytes());
            archive_bytes.extend_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            archive_bytes.extend_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.crc.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.compressed_size.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
            archive_bytes.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            archive_bytes.extend_from_slice(&0u16.to_le_bytes());
            archive_bytes.extend_from_slice(name_bytes);
            archive_bytes.extend_from_slice(&entry.compressed_data);

            central_directory.extend_from_slice(&0x02014B50u32.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&20u16.to_le_bytes());
            central_directory.extend_from_slice(&entry.flags.to_le_bytes());
            central_directory.extend_from_slice(&entry.compression_method.to_le_bytes());
            central_directory.extend_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            central_directory.extend_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());
            central_directory.extend_from_slice(&entry.crc.to_le_bytes());
            central_directory.extend_from_slice(&entry.compressed_size.to_le_bytes());
            central_directory.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
            central_directory.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&entry.external_attributes.to_le_bytes());
            central_directory.extend_from_slice(&local_offset.to_le_bytes());
            central_directory.extend_from_slice(name_bytes);
        }

        let mut comment_crc = crc32fast::Hasher::new();
        comment_crc.update(&central_directory);
        let comment = format!("TORRENTZIPPED-{:08X}", comment_crc.finalize());

        let central_directory_offset = archive_bytes.len() as u32;
        let central_directory_size = central_directory.len() as u32;
        archive_bytes.extend_from_slice(&central_directory);
        archive_bytes.extend_from_slice(&0x06054B50u32.to_le_bytes());
        archive_bytes.extend_from_slice(&0u16.to_le_bytes());
        archive_bytes.extend_from_slice(&0u16.to_le_bytes());
        archive_bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        archive_bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        archive_bytes.extend_from_slice(&central_directory_size.to_le_bytes());
        archive_bytes.extend_from_slice(&central_directory_offset.to_le_bytes());
        archive_bytes.extend_from_slice(&(comment.len() as u16).to_le_bytes());
        archive_bytes.extend_from_slice(comment.as_bytes());

        Some(archive_bytes)
    }

    fn rebuild_zip_archive(
        zip_file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) -> bool {
        let desired_zip_struct = zip_file.borrow().new_zip_struct();
        let zip_path = Self::get_existing_physical_path(Rc::clone(&zip_file));
        let temp_zip_path = format!("{}.rvfix.tmp", zip_path);
        let current_exists = Path::new(&zip_path).exists();
        let write_exact_torrentzip = matches!(desired_zip_struct, ZipStructure::ZipTrrnt);
        let mut retained_entries = 0usize;
        let mut any_changes = current_exists && zip_file.borrow().zip_struct != desired_zip_struct;
        let mut entries = Vec::new();
        Self::collect_archive_rebuild_entries(Rc::clone(&zip_file), "", "", &mut entries, &mut any_changes);
        Self::sort_archive_rebuild_entries(&mut entries, desired_zip_struct);
        let mut torrentzip_entries: Vec<TorrentZipBuiltEntry> = Vec::new();
        let mut writer = if write_exact_torrentzip {
            None
        } else {
            let temp_file = match File::create(&temp_zip_path) {
                Ok(file) => file,
                Err(_) => return false,
            };
            Some(ZipWriter::new(temp_file))
        };
        let compression_method = match desired_zip_struct {
            ZipStructure::ZipZSTD => CompressionMethod::Zstd,
            _ => CompressionMethod::Deflated,
        };
        let mut options = SimpleFileOptions::default()
            .compression_method(compression_method)
            .compression_level(Some(9));
        if let Some(date_time) = Self::torrentzip_datetime() {
            options = options.last_modified_time(date_time);
        }

        for entry in &entries {
            let (child_name, existing_child_name, rep_status, got_status, is_directory) = {
                let child_ref = entry.node.borrow();
                (
                    entry.target_name.clone(),
                    entry.existing_name.clone(),
                    child_ref.rep_status(),
                    child_ref.got_status(),
                    entry.is_directory,
                )
            };

            if is_directory {
                match rep_status {
                    RepStatus::Delete | RepStatus::UnNeeded => {
                        any_changes = true;
                        continue;
                    }
                    RepStatus::MoveToSort | RepStatus::MoveToCorrupt => {
                        let target_path = Self::get_archive_member_tosort_path(
                            Path::new(&zip_path),
                            &existing_child_name,
                            if matches!(rep_status, RepStatus::MoveToCorrupt) {
                                "ToSort/Corrupt"
                            } else {
                                "ToSort"
                            },
                        );
                        if fs::create_dir_all(&target_path).is_err() {
                            let _ = fs::remove_file(&temp_zip_path);
                            return false;
                        }
                        any_changes = true;
                        continue;
                    }
                    RepStatus::Rename => {
                        if existing_child_name != child_name {
                            any_changes = true;
                        }
                    }
                    _ => {}
                }

                if write_exact_torrentzip {
                    torrentzip_entries.push(Self::build_torrentzip_directory_entry(&child_name));
                    retained_entries += 1;
                } else if writer.as_mut().map_or(true, |writer| {
                    writer.add_directory(format!("{}/", child_name.trim_end_matches('/')), options).is_err()
                }) {
                    let _ = fs::remove_file(&temp_zip_path);
                    return false;
                } else {
                    retained_entries += 1;
                }
                continue;
            }

            let entry_bytes = match rep_status {
                RepStatus::Delete | RepStatus::UnNeeded => {
                    any_changes = true;
                    continue;
                }
                RepStatus::MoveToSort | RepStatus::MoveToCorrupt => {
                    let Some(bytes) = Self::read_zip_entry_bytes(&zip_path, &existing_child_name) else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };
                    let target_path = Self::get_archive_member_tosort_path(
                        Path::new(&zip_path),
                        &existing_child_name,
                        if matches!(rep_status, RepStatus::MoveToCorrupt) {
                            "ToSort/Corrupt"
                        } else {
                            "ToSort"
                        },
                    );
                    if fs::write(&target_path, &bytes).is_err() {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    }
                    any_changes = true;
                    continue;
                }
                RepStatus::Rename => {
                    let Some(bytes) = Self::read_zip_entry_bytes(&zip_path, &existing_child_name) else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };
                    if existing_child_name != child_name {
                        any_changes = true;
                    }
                    bytes
                }
                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                    let source_file = {
                        let child_ref = entry.node.borrow();
                        Self::find_source_file(&child_ref, crc_map, sha1_map, md5_map)
                    };
                    let Some(source_file) = source_file else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };

                    let bytes = Self::read_source_file_bytes(Rc::clone(&source_file));
                    let Some(bytes) = bytes else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };

                    let source_is_read_only = {
                        let source_ref = source_file.borrow();
                        Self::is_fix_read_only(&source_ref)
                    };
                    let source_is_same_node = Rc::ptr_eq(&source_file, &entry.node);
                    let source_is_same_archive =
                        Self::source_uses_same_archive_path(Rc::clone(&source_file), Path::new(&zip_path));

                    if write_exact_torrentzip {
                        let built_entry = Self::maybe_reuse_torrentzip_stream_from_source(Rc::clone(&source_file))
                            .or_else(|| Self::compress_torrentzip_entry(&child_name.replace('\\', "/"), &bytes));
                        let Some(built_entry) = built_entry else {
                            let _ = fs::remove_file(&temp_zip_path);
                            return false;
                        };
                        if !source_is_read_only && !source_is_same_node && !source_is_same_archive {
                            Self::queue_source_cleanup(source_file, queue);
                        }
                        any_changes = true;
                        torrentzip_entries.push(built_entry);
                        retained_entries += 1;
                        continue;
                    }

                    if !source_is_read_only && !source_is_same_node && !source_is_same_archive {
                        Self::queue_source_cleanup(source_file, queue);
                    }

                    any_changes = true;
                    bytes
                }
                _ => {
                    if !current_exists {
                        if got_status == GotStatus::Got {
                            let _ = fs::remove_file(&temp_zip_path);
                            return false;
                        }
                        continue;
                    }

                    let Some(bytes) = Self::read_zip_entry_bytes(&zip_path, &existing_child_name) else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };
                    bytes
                }
            };

            if write_exact_torrentzip {
                let built_entry = Self::maybe_reuse_torrentzip_stream_from_existing(&zip_path, &existing_child_name)
                    .or_else(|| Self::compress_torrentzip_entry(&child_name.replace('\\', "/"), &entry_bytes));
                let Some(built_entry) = built_entry else {
                    let _ = fs::remove_file(&temp_zip_path);
                    return false;
                };
                torrentzip_entries.push(built_entry);
                retained_entries += 1;
            } else if writer.as_mut().map_or(true, |writer| {
                writer.start_file(child_name, options).is_err() || writer.write_all(&entry_bytes).is_err()
            }) {
                let _ = fs::remove_file(&temp_zip_path);
                return false;
            } else {
                retained_entries += 1;
            }
        }

        if write_exact_torrentzip {
            let Some(archive_bytes) = Self::build_torrentzip_archive(&torrentzip_entries) else {
                let _ = fs::remove_file(&temp_zip_path);
                return false;
            };
            if fs::write(&temp_zip_path, archive_bytes).is_err() {
                let _ = fs::remove_file(&temp_zip_path);
                return false;
            }
        } else if writer
            .take()
            .map_or(false, |writer| writer.finish().is_err())
        {
            let _ = fs::remove_file(&temp_zip_path);
            return false;
        }

        if !any_changes {
            let _ = fs::remove_file(&temp_zip_path);
            return false;
        }

        if retained_entries == 0 {
            let _ = fs::remove_file(&temp_zip_path);
            if Path::new(&zip_path).exists() {
                let _ = fs::remove_file(&zip_path);
            }

            for entry in &entries {
                let mut child_ref = entry.node.borrow_mut();
                match child_ref.rep_status() {
                    RepStatus::Delete | RepStatus::UnNeeded => {
                        child_ref.set_got_status(GotStatus::NotGot);
                        child_ref.rep_status_reset();
                    }
                    RepStatus::MoveToSort => {
                        child_ref.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                        child_ref.rep_status_reset();
                    }
                    RepStatus::MoveToCorrupt => {
                        child_ref.set_got_status(GotStatus::NotGot);
                        child_ref.rep_status_reset();
                    }
                    _ => {}
                }
            }

            let mut zip_mut = zip_file.borrow_mut();
            zip_mut.set_got_status(GotStatus::NotGot);
            zip_mut.rep_status_reset();
            zip_mut.cached_stats = None;
            return true;
        }

        if let Some(parent) = Path::new(&zip_path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        if Path::new(&zip_path).exists() {
            let _ = fs::remove_file(&zip_path);
        }

        if fs::rename(&temp_zip_path, &zip_path).is_err() {
            let _ = fs::copy(&temp_zip_path, &zip_path);
            let _ = fs::remove_file(&temp_zip_path);
        }

        if matches!(desired_zip_struct, ZipStructure::ZipTrrnt) && !write_exact_torrentzip {
            let _ = Self::apply_torrentzip_metadata(&zip_path);
        }

        for entry in &entries {
            let mut child_ref = entry.node.borrow_mut();
            match child_ref.rep_status() {
                RepStatus::Delete | RepStatus::UnNeeded => {
                    child_ref.set_got_status(GotStatus::NotGot);
                    child_ref.rep_status_reset();
                }
                RepStatus::MoveToSort => {
                    child_ref.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                    child_ref.rep_status_reset();
                }
                RepStatus::MoveToCorrupt => {
                    child_ref.set_got_status(GotStatus::NotGot);
                    child_ref.rep_status_reset();
                }
                RepStatus::CanBeFixed => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::CanBeFixedMIA => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::CorruptCanBeFixed => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::Rename => {
                    child_ref.file_name = child_ref.name.clone();
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                }
                _ => {}
            }
        }

        let mut zip_mut = zip_file.borrow_mut();
        zip_mut.zip_struct = desired_zip_struct;
        zip_mut.set_got_status(GotStatus::Got);
        zip_mut.rep_status_reset();
        zip_mut.cached_stats = None;
        true
    }

    fn sort_archive_rebuild_entries(entries: &mut [ArchiveRebuildEntry], desired_zip_struct: ZipStructure) {
        entries.sort_by(|a, b| {
            let a_ref = a.node.borrow();
            let b_ref = b.node.borrow();
            let zf_a = ZippedFile {
                index: 0,
                name: a.target_name.clone(),
                size: a_ref.size.unwrap_or(0),
                crc: a_ref.crc.clone(),
                sha1: a_ref.sha1.clone(),
                is_dir: a.is_directory,
            };
            let zf_b = ZippedFile {
                index: 0,
                name: b.target_name.clone(),
                size: b_ref.size.unwrap_or(0),
                crc: b_ref.crc.clone(),
                sha1: b_ref.sha1.clone(),
                is_dir: b.is_directory,
            };

            let cmp = match desired_zip_struct {
                ZipStructure::SevenZipTrrnt
                | ZipStructure::SevenZipSLZMA
                | ZipStructure::SevenZipNLZMA
                | ZipStructure::SevenZipSZSTD
                | ZipStructure::SevenZipNZSTD => TorrentZipCheck::trrnt_7zip_string_compare(&zf_a, &zf_b),
                _ => TorrentZipCheck::trrnt_zip_string_compare(&zf_a, &zf_b),
            };

            cmp.cmp(&0)
        });
    }

    fn rebuild_seven_zip_archive(
        archive_file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) -> bool {
        let desired_zip_struct = archive_file.borrow().new_zip_struct();
        let archive_path = Self::get_existing_physical_path(Rc::clone(&archive_file));
        let temp_archive_path = format!("{}.rvfix.tmp", archive_path);
        let staging_dir = PathBuf::from(format!("{}.rvfix.dir", archive_path));
        let current_exists = Path::new(&archive_path).exists();
        let mut retained_entries = 0usize;
        let mut any_changes = current_exists && archive_file.borrow().zip_struct != desired_zip_struct;
        let mut entries = Vec::new();
        Self::collect_archive_rebuild_entries(Rc::clone(&archive_file), "", "", &mut entries, &mut any_changes);
        Self::sort_archive_rebuild_entries(&mut entries, desired_zip_struct);

        let _ = fs::remove_dir_all(&staging_dir);
        if fs::create_dir_all(&staging_dir).is_err() {
            return false;
        }

        for entry in &entries {
            let (child_name, existing_child_name, rep_status, got_status, is_directory) = {
                let child_ref = entry.node.borrow();
                (
                    entry.target_name.clone(),
                    entry.existing_name.clone(),
                    child_ref.rep_status(),
                    child_ref.got_status(),
                    entry.is_directory,
                )
            };

            if is_directory {
                let _ = fs::remove_dir_all(&staging_dir);
                return false;
            }

            let entry_bytes = match rep_status {
                RepStatus::Delete | RepStatus::UnNeeded => {
                    any_changes = true;
                    continue;
                }
                RepStatus::MoveToSort | RepStatus::MoveToCorrupt => {
                    let Some(bytes) = Self::read_seven_zip_entry_bytes(&archive_path, &existing_child_name) else {
                        let _ = fs::remove_dir_all(&staging_dir);
                        return false;
                    };
                    let target_path = Self::get_archive_member_tosort_path(
                        Path::new(&archive_path),
                        &existing_child_name,
                        if matches!(rep_status, RepStatus::MoveToCorrupt) {
                            "ToSort/Corrupt"
                        } else {
                            "ToSort"
                        },
                    );
                    if fs::write(&target_path, &bytes).is_err() {
                        let _ = fs::remove_dir_all(&staging_dir);
                        return false;
                    }
                    any_changes = true;
                    continue;
                }
                RepStatus::Rename => {
                    let Some(bytes) = Self::read_seven_zip_entry_bytes(&archive_path, &existing_child_name) else {
                        let _ = fs::remove_dir_all(&staging_dir);
                        return false;
                    };
                    if existing_child_name != child_name {
                        any_changes = true;
                    }
                    bytes
                }
                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                    let source_file = {
                        let child_ref = entry.node.borrow();
                        Self::find_source_file(&child_ref, crc_map, sha1_map, md5_map)
                    };
                    let Some(source_file) = source_file else {
                        let _ = fs::remove_dir_all(&staging_dir);
                        return false;
                    };

                    let Some(bytes) = Self::read_source_file_bytes(Rc::clone(&source_file)) else {
                        let _ = fs::remove_dir_all(&staging_dir);
                        return false;
                    };

                    let source_is_read_only = {
                        let source_ref = source_file.borrow();
                        Self::is_fix_read_only(&source_ref)
                    };
                    let source_is_same_node = Rc::ptr_eq(&source_file, &entry.node);
                    let source_is_same_archive =
                        Self::source_uses_same_archive_path(Rc::clone(&source_file), Path::new(&archive_path));

                    if !source_is_read_only && !source_is_same_node && !source_is_same_archive {
                        Self::queue_source_cleanup(source_file, queue);
                    }

                    any_changes = true;
                    bytes
                }
                _ => {
                    if !current_exists {
                        if got_status == GotStatus::Got {
                            let _ = fs::remove_dir_all(&staging_dir);
                            return false;
                        }
                        continue;
                    }

                    let Some(bytes) = Self::read_seven_zip_entry_bytes(&archive_path, &existing_child_name) else {
                        let _ = fs::remove_dir_all(&staging_dir);
                        return false;
                    };
                    bytes
                }
            };

            let staged_path = staging_dir.join(&child_name);
            if let Some(parent) = staged_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::write(&staged_path, &entry_bytes).is_err() {
                let _ = fs::remove_dir_all(&staging_dir);
                return false;
            }
            retained_entries += 1;
        }

        if !any_changes {
            let _ = fs::remove_dir_all(&staging_dir);
            return false;
        }

        if retained_entries == 0 {
            let _ = fs::remove_dir_all(&staging_dir);
            let _ = fs::remove_file(&temp_archive_path);
            if Path::new(&archive_path).exists() {
                let _ = fs::remove_file(&archive_path);
            }

            for entry in &entries {
                let mut child_ref = entry.node.borrow_mut();
                match child_ref.rep_status() {
                    RepStatus::Delete | RepStatus::UnNeeded => {
                        child_ref.set_got_status(GotStatus::NotGot);
                        child_ref.rep_status_reset();
                    }
                    RepStatus::MoveToSort => {
                        child_ref.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                        child_ref.rep_status_reset();
                    }
                    RepStatus::MoveToCorrupt => {
                        child_ref.set_got_status(GotStatus::NotGot);
                        child_ref.rep_status_reset();
                    }
                    _ => {}
                }
            }

            let mut archive_mut = archive_file.borrow_mut();
            archive_mut.set_got_status(GotStatus::NotGot);
            archive_mut.rep_status_reset();
            archive_mut.cached_stats = None;
            return true;
        }

        let _ = fs::remove_file(&temp_archive_path);
        if compress_to_7z_path(&staging_dir, &temp_archive_path).is_err() {
            let _ = fs::remove_dir_all(&staging_dir);
            let _ = fs::remove_file(&temp_archive_path);
            return false;
        }

        if Path::new(&archive_path).exists() {
            let _ = fs::remove_file(&archive_path);
        }

        if fs::rename(&temp_archive_path, &archive_path).is_err() {
            let _ = fs::copy(&temp_archive_path, &archive_path);
            let _ = fs::remove_file(&temp_archive_path);
        }

        for entry in &entries {
            let mut child_ref = entry.node.borrow_mut();
            match child_ref.rep_status() {
                RepStatus::Delete | RepStatus::UnNeeded => {
                    child_ref.set_got_status(GotStatus::NotGot);
                    child_ref.rep_status_reset();
                }
                RepStatus::MoveToSort => {
                    child_ref.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                    child_ref.rep_status_reset();
                }
                RepStatus::MoveToCorrupt => {
                    child_ref.set_got_status(GotStatus::NotGot);
                    child_ref.rep_status_reset();
                }
                RepStatus::CanBeFixed => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::CanBeFixedMIA => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::CorruptCanBeFixed => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::Rename => {
                    child_ref.file_name = child_ref.name.clone();
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                }
                _ => {}
            }
        }

        let mut archive_mut = archive_file.borrow_mut();
        archive_mut.zip_struct = desired_zip_struct;
        archive_mut.set_got_status(GotStatus::Got);
        archive_mut.rep_status_reset();
        archive_mut.cached_stats = None;

        let _ = fs::remove_dir_all(&staging_dir);
        true
    }

    fn try_zip_move(
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
            let _ = fs::create_dir_all(parent);
        }

        if Path::new(&target_archive_path).exists() {
            let _ = fs::remove_file(&target_archive_path);
        }

        let source_is_read_only = {
            let source_archive_ref = source_archive.borrow();
            Self::is_fix_read_only(&source_archive_ref)
        };

        let moved_ok = if source_is_read_only {
            fs::copy(&source_archive_path, &target_archive_path).is_ok()
        } else {
            fs::rename(&source_archive_path, &target_archive_path).is_ok()
                || (fs::copy(&source_archive_path, &target_archive_path).is_ok()
                    && fs::remove_file(&source_archive_path).is_ok())
        };

        if !moved_ok {
            return false;
        }

        Self::mark_tree_as_got(Rc::clone(&zip_file));
        *total_fixed += 1;

        if !source_is_read_only {
            source_archive.borrow_mut().set_rep_status(RepStatus::Delete);
            queue.push(source_archive);
        }

        true
    }

    fn fix_a_zip(
        zip_file: Rc<RefCell<RvFile>>, 
        queue: &mut Vec<Rc<RefCell<RvFile>>>, 
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) {
        if Self::try_zip_move(
            Rc::clone(&zip_file),
            queue,
            total_fixed,
            crc_map,
            sha1_map,
            md5_map,
        ) {
            return;
        }

        if zip_file.borrow().file_type == FileType::SevenZip
            && Self::rebuild_seven_zip_archive(
                Rc::clone(&zip_file),
                queue,
                total_fixed,
                crc_map,
                sha1_map,
                md5_map,
            )
        {
            return;
        }

        if Self::rebuild_zip_archive(
            Rc::clone(&zip_file),
            queue,
            total_fixed,
            crc_map,
            sha1_map,
            md5_map,
        ) {
            return;
        }

        // C# FixAZipCore logic stub.
        // C# uses a virtual stream pipeline here. We will fall back to processing
        // its children as standard files, simulating basic Zip IO via `fix_a_file`.
        let children = zip_file.borrow().children.clone();
        for child in children {
            Self::fix_a_file(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map);
        }
    }

    fn fix_a_file(
        file: Rc<RefCell<RvFile>>, 
        queue: &mut Vec<Rc<RefCell<RvFile>>>, 
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) {
        let (rep_status, name, current_path, target_path, is_read_only) = {
            let file_ref = file.borrow();
            let current_path = Self::build_physical_path(Rc::clone(&file), true);
            let target_path = Self::build_physical_path(Rc::clone(&file), false);
            (
                file_ref.rep_status(),
                file_ref.name.clone(),
                current_path,
                target_path,
                Self::is_fix_read_only(&file_ref),
            )
        };

        if is_read_only {
            return;
        }
        
        match rep_status {
            RepStatus::Delete | RepStatus::UnNeeded => {
                debug!("Deleting file: {}", current_path.display());
                let root = Self::find_tree_root(Rc::clone(&file));
                if Self::has_retained_shared_physical_path(root, Rc::clone(&file), &current_path) {
                    let mut file_mut = file.borrow_mut();
                    file_mut.set_got_status(GotStatus::NotGot);
                    file_mut.rep_status_reset();
                    return;
                }
                if current_path.exists() {
                    let _ = fs::remove_file(&current_path);
                    
                    let mut current_dir = current_path.parent();
                    while let Some(parent) = current_dir {
                        if fs::remove_dir(parent).is_err() { break; }
                        current_dir = parent.parent();
                    }
                }
                let mut file_mut = file.borrow_mut();
                file_mut.set_got_status(GotStatus::NotGot);
                file_mut.rep_status_reset();
            },
            RepStatus::MoveToSort => {
                let current_path_str = current_path.to_string_lossy();
                let tosort_path = Self::get_tosort_path(&current_path_str, "ToSort");
                let tosort_path = PathBuf::from(tosort_path);
                debug!("Moving to ToSort: {} -> {}", current_path.display(), tosort_path.display());
                if current_path.exists() {
                    let _ = Self::rename_path_if_needed(&current_path, &tosort_path, "tmptosort");
                }
                let mut file_mut = file.borrow_mut();
                file_mut.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                file_mut.rep_status_reset();
            },
            RepStatus::MoveToCorrupt => {
                let current_path_str = current_path.to_string_lossy();
                let tosort_path = Self::get_tosort_path(&current_path_str, "ToSort/Corrupt");
                let tosort_path = PathBuf::from(tosort_path);
                debug!(
                    "Moving corrupt file to ToSort/Corrupt: {} -> {}",
                    current_path.display(),
                    tosort_path.display()
                );
                if current_path.exists() {
                    let _ = Self::rename_path_if_needed(&current_path, &tosort_path, "tmptosort");
                }
                let mut file_mut = file.borrow_mut();
                file_mut.set_got_status(GotStatus::NotGot);
                file_mut.rep_status_reset();
            },
            RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                let source_file = {
                    let f = file.borrow();
                    Self::find_source_file(&f, crc_map, sha1_map, md5_map)
                };

                if let Some(src) = source_file {
                    let src_path = Self::build_physical_path(Rc::clone(&src), true);
                    
                    if Self::physical_path_eq_for_rename(&src_path, &target_path) {
                        let _ = Self::rename_path_if_needed(&src_path, &target_path, "tmpfix");
                    } else {
                        debug!("Fixing file from source: {} -> {}", src_path.display(), target_path.display());
                        
                        if let Some(parent) = target_path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }

                        if let Some(bytes) = Self::read_source_file_bytes(Rc::clone(&src)) {
                            let _ = fs::write(&target_path, bytes);
                        }
                        
                        let source_is_read_only = {
                            let src_ref = src.borrow();
                            Self::is_fix_read_only(&src_ref)
                        };

                        if !source_is_read_only {
                            Self::queue_source_cleanup(Rc::clone(&src), queue);
                        }
                    }

                    let mut file_mut = file.borrow_mut();
                    file_mut.set_got_status(GotStatus::Got);
                    file_mut.rep_status_reset();
                    *total_fixed += 1;
                } else {
                    trace!("Could not find source file for: {}", name);
                }
            },
            RepStatus::Rename => {
                debug!("Renaming file: {} -> {}", current_path.display(), target_path.display());
                let _ = Self::rename_path_if_needed(&current_path, &target_path, "tmpfile");
                {
                    let mut file_mut = file.borrow_mut();
                    file_mut.file_name = file_mut.name.clone();
                    file_mut.set_got_status(GotStatus::Got);
                    file_mut.rep_status_reset();
                }
            },
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "tests/fix_tests.rs"]
mod tests;
