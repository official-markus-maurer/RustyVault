use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use dat_reader::enums::{DatStatus, FileType, GotStatus};
use tracing::{info, trace};
use trrntzip::process_control::ProcessControl;

use crate::enums::RepStatus;
use crate::rv_file::{RvFile, TreeSelect};

/// The logical matching engine for resolving missing ROMs.
///
/// `FindFixes` is responsible for calculating the logical repair state (`RepStatus`) of the
/// database. It identifies missing files in the primary `RustyRoms` and attempts to map them
/// to available files sitting in `ToSort` using exact CRC/SHA1/MD5 hash matching.
///
/// Implementation notes:
/// - Uses `rayon` to parallelize index construction across available CPU cores.
pub struct FindFixes;

include!("find_fixes/selection.rs");
include!("find_fixes/physical_paths.rs");
include!("find_fixes/shared_paths.rs");
include!("find_fixes/matching.rs");
include!("find_fixes/physical_scan.rs");
include!("find_fixes/apply.rs");

#[cfg(test)]
#[path = "tests/find_fixes_tests.rs"]
mod tests;
