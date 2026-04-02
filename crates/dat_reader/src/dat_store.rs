use crate::enums::{DatStatus, FileType, HeaderFileType, ZipStructure};

pub const TRRNTZIP_DOS_DATETIME: i64 = ((8600u32 as i64) << 16) | 48128i64;

/// In-memory Abstract Syntax Tree (AST) for parsed DAT files.
/// 
/// `dat_store.rs` defines the hierarchical structures (`DatHeader`, `DatNode`, `DatDir`, `DatFile`, `DatGame`)
/// that represent the raw parsed contents of an XML/CMP DAT file before it is folded into the 
/// core `rv_core::DB` file tree.
/// 
/// Differences from C#:
/// - The C# `DatReader` directly interacts with the core `RvFile` and `RvDir` database nodes.
/// - The Rust version completely decouples the parsing phase from the database representation,
///   constructing an intermediate AST (`DatNode`) which is later safely merged by `rv_core::read_dat::DatUpdate`.
///   This decoupling is what enables the Rust port to parse multiple DAT files entirely in parallel!
#[derive(Debug, Clone)]
pub struct DatNode {
    pub name: String,
    pub dat_status: DatStatus,
    pub file_type: FileType,
    pub date_modified: Option<i64>,
    pub node: DatBase,
}

impl DatNode {
    pub fn new_dir(name: String, file_type: FileType) -> Self {
        DatNode {
            name,
            dat_status: DatStatus::InDatCollect,
            file_type,
            date_modified: None,
            node: DatBase::Dir(DatDir::new()),
        }
    }

    pub fn new_file(name: String, file_type: FileType) -> Self {
        DatNode {
            name,
            dat_status: DatStatus::InDatCollect,
            file_type,
            date_modified: None,
            node: DatBase::File(DatFile::new()),
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self.node, DatBase::Dir(_))
    }

    pub fn is_file(&self) -> bool {
        matches!(self.node, DatBase::File(_))
    }

    pub fn dir(&self) -> Option<&DatDir> {
        if let DatBase::Dir(ref d) = self.node {
            Some(d)
        } else {
            None
        }
    }

    pub fn dir_mut(&mut self) -> Option<&mut DatDir> {
        if let DatBase::Dir(ref mut d) = self.node {
            Some(d)
        } else {
            None
        }
    }

    pub fn file(&self) -> Option<&DatFile> {
        if let DatBase::File(ref f) = self.node {
            Some(f)
        } else {
            None
        }
    }

    pub fn file_mut(&mut self) -> Option<&mut DatFile> {
        if let DatBase::File(ref mut f) = self.node {
            Some(f)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum DatBase {
    Dir(DatDir),
    File(DatFile),
}

#[derive(Debug, Clone, Default)]
pub struct DatDir {
    dat_struct: u8,
    pub d_game: Option<Box<DatGame>>,
    pub children: Vec<DatNode>,
}

impl DatDir {
    pub fn new() -> Self {
        Self {
            dat_struct: 0,
            d_game: None,
            children: Vec::new(),
        }
    }

    pub fn dat_struct(&self) -> ZipStructure {
        ZipStructure::from(self.dat_struct & 0x7f)
    }

    pub fn dat_struct_fix(&self) -> bool {
        (self.dat_struct & 0x80) == 0x80
    }

    pub fn set_dat_struct(&mut self, zip_structure: ZipStructure, fix: bool) {
        self.dat_struct = zip_structure as u8;
        if fix {
            self.dat_struct |= 0x80;
        }
    }

    pub fn add_child(&mut self, child: DatNode) {
        // We'll skip the complex binary search index logic from C# for now 
        // and just push to the vector. In a real-world Rust optimization, 
        // we might sort this or use a BTreeMap if `name` lookups are frequent.
        self.children.push(child);
    }
}

#[derive(Debug, Clone, Default)]
pub struct DatFile {
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub sha256: Option<Vec<u8>>,
    pub merge: Option<String>,
    pub status: Option<String>,
    pub region: Option<String>,
    pub mia: Option<String>,
    pub is_disk: bool,
    pub header_file_type: HeaderFileType,
}

impl DatFile {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct DatGame {
    pub id: Option<String>,
    pub description: Option<String>,
    pub manufacturer: Option<String>,
    pub history: Option<String>,
    pub clone_of: Option<String>,
    pub clone_of_id: Option<String>,
    pub rom_of: Option<String>,
    pub sample_of: Option<String>,
    pub source_file: Option<String>,
    pub is_bios: Option<String>,
    pub is_device: Option<String>,
    pub board: Option<String>,
    pub year: Option<String>,
    pub runnable: Option<String>,

    pub category: Vec<String>,
    pub device_ref: Vec<String>,

    pub is_emu_arc: bool,
    pub publisher: Option<String>,
    pub developer: Option<String>,
    pub genre: Option<String>,
    pub sub_genre: Option<String>,
    pub ratings: Option<String>,
    pub score: Option<String>,
    pub players: Option<String>,
    pub enabled: Option<String>,
    pub crc: Option<String>,
    pub source: Option<String>,
    pub related_to: Option<String>,

    pub game_hash: Option<Vec<u8>>,
    pub found: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DatHeader {
    pub id: Option<String>,
    pub filename: Option<String>,
    pub mame_xml: bool,
    pub name: Option<String>,
    pub type_: Option<String>, // type is reserved keyword
    pub root_dir: Option<String>,
    pub description: Option<String>,
    pub subset: Option<String>,
    pub category: Option<String>,
    pub version: Option<String>,
    pub date: Option<String>,
    pub author: Option<String>,
    pub email: Option<String>,
    pub homepage: Option<String>,
    pub url: Option<String>,
    pub comment: Option<String>,
    pub header: Option<String>,
    pub compression: Option<String>,
    pub merge_type: Option<String>,
    pub split: Option<String>,
    pub no_dump: Option<String>,
    pub dir: Option<String>,
    pub not_zipped: bool,

    pub base_dir: DatDir,
}
