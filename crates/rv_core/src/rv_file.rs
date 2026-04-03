use std::rc::{Rc, Weak};
use std::cell::RefCell;
use dat_reader::enums::{DatStatus, FileType, GotStatus, HeaderFileType, ZipStructure};
use crate::enums::{RepStatus, ReportStatus, ToSortDirType};
use crate::repair_status::RepairStatus;
use crate::rv_dat::RvDat;
use crate::rv_game::RvGame;

bitflags::bitflags! {
    /// Bitflags representing the operational states of an `RvFile` node.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
    pub struct FileStatus: u32 {
        /// No flags set
        const NONE = 0;
        
        /// Size metadata was sourced from an archive header
        const SIZE_FROM_HEADER = 1 << 0;
        /// CRC32 metadata was sourced from an archive header
        const CRC_FROM_HEADER = 1 << 1;
        /// SHA1 metadata was sourced from an archive header
        const SHA1_FROM_HEADER = 1 << 2;
        /// MD5 metadata was sourced from an archive header
        const MD5_FROM_HEADER = 1 << 3;
        
        /// Alternate Size metadata was sourced from an archive header
        const ALT_SIZE_FROM_HEADER = 1 << 4;
        /// Alternate CRC32 metadata was sourced from an archive header
        const ALT_CRC_FROM_HEADER = 1 << 5;
        /// Alternate SHA1 metadata was sourced from an archive header
        const ALT_SHA1_FROM_HEADER = 1 << 6;
        /// Alternate MD5 metadata was sourced from an archive header
        const ALT_MD5_FROM_HEADER = 1 << 7;

        /// Size metadata was specified by the DAT
        const SIZE_FROM_DAT = 1 << 8;
        /// CRC32 metadata was specified by the DAT
        const CRC_FROM_DAT = 1 << 9;
        /// SHA1 metadata was specified by the DAT
        const SHA1_FROM_DAT = 1 << 10;
        /// MD5 metadata was specified by the DAT
        const MD5_FROM_DAT = 1 << 11;
        /// SHA256 metadata was specified by the DAT
        const SHA256_FROM_DAT = 1 << 20;
        /// SHA256 metadata was sourced from an archive header
        const SHA256_FROM_HEADER = 1 << 22;
        
        /// Alternate Size metadata was specified by the DAT
        const ALT_SIZE_FROM_DAT = 1 << 12;
        /// Alternate CRC32 metadata was specified by the DAT
        const ALT_CRC_FROM_DAT = 1 << 13;
        /// Alternate SHA1 metadata was specified by the DAT
        const ALT_SHA1_FROM_DAT = 1 << 14;
        /// Alternate MD5 metadata was specified by the DAT
        const ALT_MD5_FROM_DAT = 1 << 15;
        /// Alternate SHA256 metadata was specified by the DAT
        const ALT_SHA256_FROM_DAT = 1 << 21;
        /// Alternate SHA256 metadata was sourced from an archive header
        const ALT_SHA256_FROM_HEADER = 1 << 23;

        /// Date was specified by the DAT
        const DATE_FROM_DAT = 1 << 16;
        /// Header format was specified by the DAT
        const HEADER_FILE_TYPE_FROM_DAT = 1 << 17;
        /// Header format was parsed from the header itself
        const HEADER_FILE_TYPE_FROM_HEADER = 1 << 18;

        /// File is an alternate file
        const IS_ALT_FILE = 1 << 19;
    }
}

/// Core data structure representing a node in the RomVault file tree.
/// 
/// This is the Rust equivalent of the C# `RvFile` class. It unifies properties for 
/// both directories (`RvDir` in C# logic) and files into a single struct, using 
/// `FileType` to distinguish behavior.
/// 
/// Differences from C#:
/// - Tree pointers (`parent`, `children`) are modeled using `Rc<RefCell<RvFile>>` and 
///   `Weak<RefCell<RvFile>>` to ensure memory safety without leaking memory.
/// - Serde logic replaces C#'s `BinaryReader`/`BinaryWriter` for cache saving.
/// - UI state (e.g. `tree_expanded`, `tree_checked`) is embedded directly to support egui,
///   whereas C# often binds these to separate UI control objects (`RvTreeRow`).
#[derive(serde::Serialize, serde::Deserialize)]
pub struct RvFile {
    /// The canonical name of the file or directory.
    pub name: String,

    /// Temporary index used during cache deserialization
    pub dat_index_for_serde: Option<i32>,

    /// Internal file ID
    pub db_id: Option<i32>,
    /// Index relative to parent directory
    pub parent_index: i32,
    /// Temporary search flag
    pub search_found: bool,
    /// Tree selection state (for UI)
    pub tree_checked: TreeSelect,
    /// Tree expansion state (for UI)
    pub tree_expanded: bool,

    /// Weak reference to the parent `RvFile`
    #[serde(skip)]
    pub parent: Option<Weak<RefCell<RvFile>>>,
    /// Vector of child `RvFile`s
    pub children: Vec<Rc<RefCell<RvFile>>>,

    /// The name of the file
    pub file_name: String,
    /// Logical status within a DAT
    pub dat_status: DatStatus,
    /// Physical presence status
    pub got_status: GotStatus,
    /// Computed repair status
    pub rep_status: RepStatus,
    /// Directory status (if this is a directory)
    pub dir_status: Option<ReportStatus>,

    /// Operational bitflags
    pub file_status: FileStatus,
    /// Categorization if this file is in ToSort
    pub to_sort_type: ToSortDirType,
    /// The physical format of this file/directory
    pub file_type: FileType,
    /// The internal archive format (if applicable)
    pub zip_struct: ZipStructure,

    /// Associated metadata if this file is a Game root
    pub game: Option<Rc<RefCell<RvGame>>>,
    /// Associated metadata if this file is a DAT root
    pub dat: Option<Rc<RefCell<RvDat>>>,
    /// Vector of DATs associated with this directory
    pub dir_dats: Vec<Rc<RefCell<RvDat>>>,
    /// Clean display name for the UI
    pub ui_display_name: String,

    /// Emulation header format
    pub header_file_type: HeaderFileType,
    /// File timestamp
    pub file_mod_time_stamp: i64,
    /// Offset inside an archive
    pub local_header_offset: Option<u64>,
    /// Total byte size
    pub size: Option<u64>,
    /// CRC32 Hash
    pub crc: Option<Vec<u8>>,
    /// SHA1 Hash
    pub sha1: Option<Vec<u8>>,
    /// MD5 Hash
    pub md5: Option<Vec<u8>>,
    /// SHA256 Hash
    #[serde(default)]
    pub sha256: Option<Vec<u8>>,
    /// Headerless size
    pub alt_size: Option<u64>,
    /// Headerless CRC32
    pub alt_crc: Option<Vec<u8>>,
    /// Headerless SHA1
    pub alt_sha1: Option<Vec<u8>>,
    /// Headerless MD5
    pub alt_md5: Option<Vec<u8>>,
    /// Headerless SHA256
    #[serde(default)]
    pub alt_sha256: Option<Vec<u8>>,
    /// MAME CHD Version
    pub chd_version: Option<u32>,
    /// File Merge logic
    pub merge: String,
    /// File status string
    pub status: Option<String>,

    /// Internal zip validation struct flag
    pub zip_dat_struct: u8,
    /// Error status code
    pub error_status: u8,

    /// Cached repair status stats
    #[serde(skip)]
    pub cached_stats: Option<RepairStatus>,
    
    // Legacy cache fields
    /// Legacy tmp_dat_index
    pub tmp_dat_index: Option<i32>,
}

/// Enumeration for UI tree row checkbox states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TreeSelect {
    /// Row is not selected
    UnSelected,
    /// Row is selected
    Selected,
    /// Row is locked (cannot be selected)
    Locked,
}

impl RvFile {
    fn ascii_lower(byte: u8) -> u8 {
        if byte >= b'A' && byte <= b'Z' {
            byte + 0x20
        } else {
            byte
        }
    }

    fn trrnt_zip_string_compare(a: &str, b: &str) -> std::cmp::Ordering {
        let bytes_a = a.as_bytes();
        let bytes_b = b.as_bytes();
        let len = std::cmp::min(bytes_a.len(), bytes_b.len());

        for i in 0..len {
            let ca = Self::ascii_lower(bytes_a[i]);
            let cb = Self::ascii_lower(bytes_b[i]);
            match ca.cmp(&cb) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
        }

        bytes_a.len().cmp(&bytes_b.len())
    }

    fn trrnt_zip_string_compare_case(a: &str, b: &str) -> std::cmp::Ordering {
        let res = Self::trrnt_zip_string_compare(a, b);
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        a.cmp(b)
    }

    fn split_7zip_filename(filename: &str) -> (&str, &str, &str) {
        let dir_index = filename.rfind('/');
        let (path, name) = if let Some(i) = dir_index {
            (&filename[..i], &filename[i + 1..])
        } else {
            ("", filename)
        };

        let ext_index = name.rfind('.');
        if let Some(i) = ext_index {
            (path, &name[..i], &name[i + 1..])
        } else {
            (path, name, "")
        }
    }

    fn trrnt_7zip_string_compare(a: &str, b: &str) -> std::cmp::Ordering {
        let (path_a, name_a, ext_a) = Self::split_7zip_filename(a);
        let (path_b, name_b, ext_b) = Self::split_7zip_filename(b);

        match ext_a.cmp(ext_b) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match name_a.cmp(name_b) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        path_a.cmp(path_b)
    }

    fn directory_name_compare(a: &str, b: &str) -> std::cmp::Ordering {
        a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())
    }

    fn compare_name_key(f1: FileType, name1: &str, f2: FileType, name2: &str) -> std::cmp::Ordering {
        if f1 == FileType::FileZip || f2 == FileType::FileZip {
            return Self::trrnt_zip_string_compare_case(name1, name2);
        }
        if f1 == FileType::FileSevenZip || f2 == FileType::FileSevenZip {
            return Self::trrnt_7zip_string_compare(name1, name2);
        }

        let res = Self::directory_name_compare(name1, name2);
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        f1.cmp(&f2)
    }

    fn ordering_to_i32(ordering: std::cmp::Ordering) -> i32 {
        match ordering {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    pub fn child_name_search(&self, file_type: FileType, name: &str) -> (i32, usize) {
        let mut bottom = 0usize;
        let mut top = self.children.len();
        let mut mid = 0usize;
        let mut res = -1i32;

        while bottom < top && res != 0 {
            mid = (bottom + top) / 2;
            let mid_key = {
                let mid_ref = self.children[mid].borrow();
                Self::compare_name_key(file_type, name, mid_ref.file_type, &mid_ref.name)
            };
            res = Self::ordering_to_i32(mid_key);
            if res < 0 {
                top = mid;
            } else if res > 0 {
                bottom = mid + 1;
            }
        }

        let mut index = mid;
        if res == 0 {
            while index > 0 {
                let prev_key = {
                    let prev_ref = self.children[index - 1].borrow();
                    Self::compare_name_key(file_type, name, prev_ref.file_type, &prev_ref.name)
                };
                if prev_key != std::cmp::Ordering::Equal {
                    break;
                }
                index -= 1;
            }
        } else if res > 0 {
            index += 1;
        }

        (res, index)
    }

    fn child_insert_index(&self, child: &RvFile) -> usize {
        let mut bottom = 0usize;
        let mut top = self.children.len();

        while bottom < top {
            let mid = (bottom + top) / 2;
            let mid_key = {
                let mid_ref = self.children[mid].borrow();
                Self::compare_name_key(child.file_type, &child.name, mid_ref.file_type, &mid_ref.name)
            };
            if mid_key == std::cmp::Ordering::Greater {
                bottom = mid + 1;
            } else {
                top = mid;
            }
        }

        bottom
    }
    /// Creates a new `RvFile` of the specified `FileType` with default values.
    pub fn new(file_type: FileType) -> Self {
        Self {
            name: String::new(),
            file_name: String::new(),
            parent: None,
            dat: None,
            dat_index_for_serde: None,
            file_mod_time_stamp: i64::MIN,
            search_found: false,
            header_file_type: HeaderFileType::NOTHING,
            file_type,
            dat_status: DatStatus::NotInDat,
            got_status: GotStatus::NotGot,
            rep_status: RepStatus::UnSet,
            file_status: FileStatus::NONE,
            children: Vec::new(),
            dir_status: if file_type == FileType::Dir || file_type == FileType::Zip || file_type == FileType::SevenZip { Some(ReportStatus::Unknown) } else { None },
            zip_dat_struct: 0,
            zip_struct: ZipStructure::None,
            game: None,
            dir_dats: Vec::new(),
            ui_display_name: String::new(),
            to_sort_type: ToSortDirType::NONE,
            size: None,
            crc: None,
            sha1: None,
            md5: None,
            sha256: None,
            alt_size: None,
            alt_crc: None,
            alt_sha1: None,
            alt_md5: None,
            alt_sha256: None,
            merge: String::new(),
            status: None,
            tree_expanded: false,
            tree_checked: TreeSelect::Selected,
            cached_stats: None,
            tmp_dat_index: None,
            db_id: None,
            parent_index: -1,
            local_header_offset: None,
            chd_version: None,
            error_status: 0,
        }
    }

    /// Determines if this file acts as a logical directory container.
    pub fn is_directory(&self) -> bool {
        self.file_type == FileType::Dir || self.file_type == FileType::Zip || self.file_type == FileType::SevenZip
    }

    /// Determines if this file acts as a logical terminal file node.
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File || self.file_type == FileType::FileZip || self.file_type == FileType::FileSevenZip || self.file_type == FileType::FileOnly
    }

    /// Appends a child `RvFile` to this node's internal children vector.
    pub fn child_add(&mut self, child: Rc<RefCell<RvFile>>) {
        self.invalidate_cached_stats_with_ancestors();
        let insert_index = {
            let child_ref = child.borrow();
            self.child_insert_index(&child_ref)
        };
        self.children.insert(insert_index, child);
    }

    /// Inserts a child `RvFile` into this node's internal children vector at a specific index.
    pub fn child_insert(&mut self, index: usize, child: Rc<RefCell<RvFile>>) {
        self.invalidate_cached_stats_with_ancestors();
        self.children.insert(index, child);
    }

    /// Removes a child `RvFile` from this node's internal children vector at a specific index.
    pub fn child_remove(&mut self, index: usize) {
        if index < self.children.len() {
            self.invalidate_cached_stats_with_ancestors();
            self.children.remove(index);
        }
    }

    /// Retrieves the logical `DatStatus` of this node.
    pub fn dat_status(&self) -> DatStatus {
        self.dat_status
    }

    /// Mutates the logical `DatStatus` of this node.
    pub fn set_dat_status(&mut self, status: DatStatus) {
        self.invalidate_cached_stats_with_ancestors();
        self.dat_status = status;
    }

    /// Retrieves the physical `GotStatus` of this node.
    pub fn got_status(&self) -> GotStatus {
        self.got_status
    }

    /// Mutates the physical `GotStatus` of this node.
    pub fn set_got_status(&mut self, status: GotStatus) {
        self.invalidate_cached_stats_with_ancestors();
        self.got_status = status;
    }

    /// Retrieves the calculated `RepStatus` of this node.
    pub fn rep_status(&self) -> RepStatus {
        self.rep_status
    }

    /// Mutates the calculated `RepStatus` of this node.
    pub fn set_rep_status(&mut self, status: RepStatus) {
        self.invalidate_cached_stats_with_ancestors();
        self.rep_status = status;
    }

    fn invalidate_cached_stats_with_ancestors(&mut self) {
        self.cached_stats = None;
        if self.dir_status.is_some() {
            self.dir_status = Some(ReportStatus::Unknown);
        }
        let mut current = self.parent.as_ref().and_then(|parent| parent.upgrade());
        while let Some(node_rc) = current {
            let next = {
                let Ok(mut node) = node_rc.try_borrow_mut() else {
                    break;
                };
                node.cached_stats = None;
                if node.dir_status.is_some() {
                    node.dir_status = Some(ReportStatus::Unknown);
                }
                node.parent.as_ref().and_then(|parent| parent.upgrade())
            };
            current = next;
        }
    }

    /// Partially resets the `RepStatus` and `GotStatus` to baseline values prior to a fix pass.
    pub fn rep_status_reset(&mut self) {
        // Rust port simplification of RomVaultCore/RvDB/rvFile.cs RepStatusReset
        self.search_found = false;
        
        // When rep_status resets, the cached_stats need to be cleared
        self.invalidate_cached_stats_with_ancestors();
        
        let new_status = match (self.file_type, self.dat_status, self.got_status) {
            (
                FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatCollect,
                dat_reader::enums::GotStatus::Got
            ) => RepStatus::Correct,
            (
                FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatCollect,
                dat_reader::enums::GotStatus::NotGot
            ) => RepStatus::Missing,
            (
                FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatCollect,
                dat_reader::enums::GotStatus::Corrupt
            ) => RepStatus::Corrupt,
            (
                FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatMerged | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::NotGot
            ) => RepStatus::NotCollected,
            (
                FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatMerged | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::Got | dat_reader::enums::GotStatus::Corrupt
            ) => RepStatus::UnNeeded,
            (FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InDatMIA, dat_reader::enums::GotStatus::NotGot) => RepStatus::MissingMIA,
            (FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InDatMIA, dat_reader::enums::GotStatus::Got) => RepStatus::CorrectMIA,
            (FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InToSort, dat_reader::enums::GotStatus::Got) => RepStatus::InToSort,
            (FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InToSort, dat_reader::enums::GotStatus::NotGot) => RepStatus::Deleted,
            (FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::NotInDat, dat_reader::enums::GotStatus::Got) => RepStatus::Unknown,
            (FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::NotInDat, dat_reader::enums::GotStatus::NotGot) => RepStatus::Deleted,
            (
                FileType::Dir,
                dat_reader::enums::DatStatus::InDatCollect | dat_reader::enums::DatStatus::InDatMerged | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::Got
            ) => RepStatus::DirCorrect,
            (
                FileType::Dir,
                dat_reader::enums::DatStatus::InDatCollect | dat_reader::enums::DatStatus::InDatMerged | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::NotGot
            ) => RepStatus::DirMissing,
            (
                FileType::Dir,
                dat_reader::enums::DatStatus::InDatCollect | dat_reader::enums::DatStatus::InDatMerged | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::Corrupt
            ) => RepStatus::DirCorrupt,
            (FileType::Dir, dat_reader::enums::DatStatus::NotInDat, dat_reader::enums::GotStatus::Got) => RepStatus::DirUnknown,
            (FileType::Dir, dat_reader::enums::DatStatus::InToSort, dat_reader::enums::GotStatus::Got) => RepStatus::DirInToSort,
            _ => RepStatus::UnScanned,
        };
        self.rep_status = new_status;
    }

    /// Sets a specific `ToSortDirType` bitflag on this node.
    pub fn to_sort_status_set(&mut self, status: ToSortDirType) {
        self.to_sort_type |= status;
    }

    /// Checks if a specific `ToSortDirType` bitflag is set on this node.
    pub fn to_sort_status_is(&self, status: ToSortDirType) -> bool {
        self.to_sort_type.contains(status)
    }

    /// Clears a specific `ToSortDirType` bitflag from this node.
    pub fn to_sort_status_clear(&mut self, status: ToSortDirType) {
        self.to_sort_type.remove(status);
    }

    /// Retrieves the `HeaderFileType` associated with this node.
    pub fn header_file_type(&self) -> HeaderFileType {
        self.header_file_type & HeaderFileType::HEADER_MASK
    }

    /// Determines if a header is explicitly required based on the `HeaderFileType`.
    pub fn header_file_type_required(&self) -> bool {
        self.header_file_type.contains(HeaderFileType::REQUIRED)
    }

    /// Sets the `HeaderFileType` of this node.
    pub fn set_header_file_type(&mut self, val: HeaderFileType) {
        self.header_file_type = val;
    }

    /// Retrieves the raw `zip_dat_struct` enum representing the archive's internal state.
    pub fn zip_dat_struct(&self) -> ZipStructure {
        ZipStructure::from(self.zip_dat_struct & 0x7f)
    }

    /// Checks if this node is flagged as a Zip file requiring structural modification.
    pub fn zip_dat_struct_fix(&self) -> bool {
        (self.zip_dat_struct & 0x80) == 0x80
    }

    /// Mutates the underlying Zip structure type and validation flag.
    pub fn set_zip_dat_struct(&mut self, zip_structure: ZipStructure, fix: bool) {
        self.zip_dat_struct = zip_structure as u8;
        if fix {
            self.zip_dat_struct |= 0x80;
        }
    }

    /// Helper to convert the raw struct enum to a standard `ZipStructure`.
    pub fn new_zip_struct(&self) -> ZipStructure {
        if self.dat_status == DatStatus::NotInDat || self.dat_status == DatStatus::InToSort {
            self.zip_struct
        } else {
            self.zip_dat_struct()
        }
    }

    /// Modifies the `FileStatus` bitflags of this node.
    pub fn file_status_set(&mut self, flag: FileStatus) {
        self.file_status |= flag;
    }

    /// Clears specific `FileStatus` bitflags from this node.
    pub fn file_status_clear(&mut self, flag: FileStatus) {
        self.file_status.remove(flag);
    }

    /// Checks if specific `FileStatus` bitflags are set on this node.
    pub fn file_status_is(&self, flag: FileStatus) -> bool {
        self.file_status.contains(flag)
    }

    /// Batch mutates the logical `DatStatus` and physical `GotStatus`.
    pub fn set_dat_got_status(&mut self, dat: DatStatus, got: GotStatus) {
        self.invalidate_cached_stats_with_ancestors();
        self.dat_status = dat;
        self.got_status = got;
    }

    /// Explicitly marks a node as missing by reverting its `GotStatus` and removing any physical file attributes.
    pub fn mark_as_missing(&mut self) {
        self.cached_stats = None;
        self.set_got_status(GotStatus::NotGot);
        
        let mut i = 0;
        while i < self.children.len() {
            let child_rc = Rc::clone(&self.children[i]);
            let should_remove = {
                let mut child = child_rc.borrow_mut();
                child.cached_stats = None;
                child.set_got_status(GotStatus::NotGot);
                
                if child.dat_status() == DatStatus::NotInDat || child.dat_status() == DatStatus::InToSort {
                    child.file_remove()
                } else {
                    false
                }
            };

            if should_remove {
                self.children.remove(i);
            } else {
                let mut child = self.children[i].borrow_mut();
                if child.is_directory() {
                    child.mark_as_missing();
                }
                child.rep_status_reset();
                i += 1;
            }
        }
        self.rep_status_reset();
    }

    /// Empties a directory node entirely by removing all child nodes.
    pub fn file_remove(&mut self) -> bool {
        self.cached_stats = None;
        if self.is_file() {
            return true;
        }

        self.children.clear();
        true
    }

    /// Resolves the raw path metadata of this node into a fully absolute path string
    /// on the physical disk. This traverses `self.parent` recursively up to the root.
    pub fn get_full_name(&self) -> String {
        let mut path_parts = vec![self.name.clone()];
        let mut current_parent = self.parent.as_ref().and_then(|p| p.upgrade());
        while let Some(parent) = current_parent {
            let parent_borrow = parent.borrow();
            if !parent_borrow.name.is_empty() {
                path_parts.push(parent_borrow.name.clone());
            }
            current_parent = parent_borrow.parent.as_ref().and_then(|p| p.upgrade());
        }
        path_parts.reverse();
        let logical_path = path_parts.join("\\");
        let path = std::path::PathBuf::from(&logical_path);
        if path.is_absolute() {
            return path.to_string_lossy().replace('\\', "/");
        }
        crate::settings::find_dir_mapping(&logical_path)
            .unwrap_or_else(|| logical_path.replace('\\', "/"))
    }

    /// Converts a node's filename to lowercase, ensuring exact structural matching against 
    /// the TorrentZip specification format.
    #[inline]
    pub fn name_case(&self) -> &str {
        if self.file_name.trim().is_empty() {
            &self.name
        } else {
            &self.file_name
        }
    }

    /// Safely unwraps the `parent` Weak pointer, returning a clone of the `Rc<RefCell<RvFile>>` if it exists.
    pub fn get_parent(&self) -> Option<Rc<RefCell<RvFile>>> {
        self.parent.as_ref().and_then(|p| p.upgrade())
    }

    /// Checks if this node is physically present.
    pub fn is_got(&self) -> bool {
        self.got_status == GotStatus::Got || self.got_status == GotStatus::Corrupt
    }

    /// Recursively sorts the children of this node alphabetically by their filenames.
    pub fn sort_children(&mut self) {
        self.children.sort_by(|a, b| {
            a.borrow().name_case().cmp(b.borrow().name_case())
        });
        for child in self.children.iter_mut() {
            child.borrow_mut().sort_children();
        }
    }
}

#[cfg(test)]
#[path = "tests/rv_file_tests.rs"]
mod tests;
