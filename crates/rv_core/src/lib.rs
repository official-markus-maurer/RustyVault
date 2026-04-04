//! The core logical engine of RomVault.
//! 
//! `rv_core` contains all the data structures (`RvFile`, `RvDat`, `RvGame`), the physical disk 
//! interaction engines (`Scanner`, `FileScanning`), the logical fix engine (`FindFixes`), 
//! and the actual disk mutator (`Fix`). It acts as the "backend" that powers both the CLI 
//! (`rom_vault`) and the GUI (`romvault_ui`).
//! 
//! Differences from C#:
//! - Represents the `ROMVaultCore` project from the original C# solution.

/// Module containing Core Status Enums
pub mod enums;
/// Module containing DAT definitions
pub mod rv_dat;
/// Module containing Game definitions
pub mod rv_game;
/// Module containing File node definitions
pub mod rv_file;
/// Module containing Database root logic
pub mod db;
/// Module containing Database query helpers
pub mod db_helper;
/// Module containing Scanner temporary files
pub mod scanned_file;
/// Module containing Physical disk scanning logic
pub mod scanner;
/// Module containing Logical fix pairing logic
pub mod find_fixes;
/// Module containing Physical file mutation logic
pub mod fix;
/// Module containing DAT parsing integration
pub mod read_dat;
/// Module containing XML settings definitions
pub mod settings;
/// Module containing Tree status statistics
pub mod repair_status;
/// Module containing Database syncing logic
pub mod file_scanning;
/// Module containing File comparison logic
pub mod compare;
/// Module containing DAT exporting logic
pub mod external_dat_converter_to;
/// Module containing Fix DAT report generation
pub mod fix_dat_report;
pub mod chd;
pub mod relative_path;
pub mod arr_byte;
pub mod db_type_get;
pub mod is_file_only;
pub mod ulong_utils;
pub mod clean_partial;
pub mod report_error;
pub mod task_reporter;
pub mod mia_callback;
pub mod patterns;

/// Module containing Cache serialization
pub mod cache;
pub mod byte_sorted_list;
pub mod fast_array_sort;

pub use enums::*;
pub use rv_dat::*;
pub use rv_game::*;
pub use rv_file::*;
pub use db::*;
pub use db_helper::*;
pub use scanned_file::*;
pub use scanner::*;
pub use find_fixes::*;
pub use fix::*;
pub use read_dat::*;
pub use settings::*;
pub use repair_status::*;
pub use file_scanning::*;
pub use compare::*;
pub use external_dat_converter_to::*;
pub use fix_dat_report::*;
pub use chd::*;
pub use relative_path::*;
pub use arr_byte::{
    b_compare,
    copy_bytes,
    copy_bytes_range,
    e_compare,
    i_compare as bytes_i_compare,
    read_byte_array,
    to_hex_string,
    write_byte_array,
};
pub use db_type_get::*;
pub use is_file_only::*;
pub use ulong_utils::{i_compare as u64_i_compare, i_compare_null};
pub use clean_partial::*;
pub use report_error::*;
pub use task_reporter::*;
pub use mia_callback::*;
pub use patterns::*;
pub use cache::*;
pub use byte_sorted_list::*;
pub use fast_array_sort::*;
