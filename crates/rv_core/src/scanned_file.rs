use dat_reader::enums::{FileType, GotStatus, HeaderFileType, ZipStructure};
use crate::rv_file::FileStatus;

#[derive(Clone, Debug)]
pub struct ScannedFile {
    pub name: String,
    pub file_mod_time_stamp: i64,
    pub file_type: FileType,
    pub got_status: GotStatus,

    pub zip_struct: ZipStructure,
    pub comment: String,
    pub children: Vec<ScannedFile>,

    pub local_header_offset: Option<u64>,
    pub deep_scanned: bool,
    pub status_flags: FileStatus,
    pub index: i32,

    pub header_file_type: HeaderFileType,
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub sha256: Option<Vec<u8>>,

    pub alt_size: Option<u64>,
    pub alt_crc: Option<Vec<u8>>,
    pub alt_sha1: Option<Vec<u8>>,
    pub alt_md5: Option<Vec<u8>>,
    pub alt_sha256: Option<Vec<u8>>,

    pub chd_version: Option<u32>,
    pub search_found: bool,
}

impl ScannedFile {
    pub fn new(file_type: FileType) -> Self {
        Self {
            name: String::new(),
            file_mod_time_stamp: 0,
            file_type,
            got_status: GotStatus::NotGot,
            zip_struct: ZipStructure::None,
            comment: String::new(),
            children: if file_type == FileType::Dir || file_type == FileType::Zip || file_type == FileType::SevenZip {
                Vec::new()
            } else {
                Vec::new() // Default empty
            },
            local_header_offset: None,
            deep_scanned: false,
            status_flags: FileStatus::NONE,
            index: 0,
            header_file_type: HeaderFileType::NOTHING,
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
            chd_version: None,
            search_found: false,
        }
    }

    pub fn is_directory(&self) -> bool {
        self.file_type == FileType::Dir || self.file_type == FileType::Zip || self.file_type == FileType::SevenZip
    }

    pub fn add(&mut self, child: ScannedFile) {
        self.children.push(child);
    }

    pub fn sort(&mut self) {
        // TrrntZip sorting rule: sort alphabetically by byte array representation of the filename
        self.children.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    }
}
