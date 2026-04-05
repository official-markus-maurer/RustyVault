use crate::rv_dat::{DatData, RvDat};
use crate::rv_file::{FileStatus, RvFile};
use crate::rv_game::RvGame;
use dat_reader::dat_store::{DatHeader, DatNode};
use dat_reader::enums::DatStatus;
use dat_reader::enums::{FileType, HeaderFileType};
use dat_reader::read_dat;
use rayon::prelude::*;
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

/// Central engine for reading DAT files and integrating them into the `DB` tree.
///
/// `DatUpdate` reads the physical `.dat` / `.xml` files residing in the `DatRoot` folder,
/// parses them using `dat_reader`, and translates the resulting `DatNode` hierarchies into
/// `RvFile` nodes within the `dir_root` DB tree.
///
/// Implementation notes:
/// - Parsing is parallelized with `rayon` (I/O + parse is independent per DAT).
/// - Integration into the shared `Rc<RefCell<RvFile>>` tree is performed sequentially.
pub struct DatUpdate;

impl DatUpdate {
    const PRESERVED_PHYSICAL_FLAGS: FileStatus = FileStatus::SIZE_FROM_HEADER
        .union(FileStatus::CRC_FROM_HEADER)
        .union(FileStatus::SHA1_FROM_HEADER)
        .union(FileStatus::MD5_FROM_HEADER)
        .union(FileStatus::ALT_SIZE_FROM_HEADER)
        .union(FileStatus::ALT_CRC_FROM_HEADER)
        .union(FileStatus::ALT_SHA1_FROM_HEADER)
        .union(FileStatus::ALT_MD5_FROM_HEADER)
        .union(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
}

include!("path_utils.rs");
include!("state.rs");
include!("scan.rs");
include!("mapping.rs");
include!("update.rs");
