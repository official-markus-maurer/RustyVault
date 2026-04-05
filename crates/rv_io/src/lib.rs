pub mod directory;
pub mod directory_info;
/// Cross-platform file I/O abstractions.
///
/// `rv_io` provides a small wrapper layer over `std::fs` and `std::path` with helpers used by the
/// rest of the workspace (e.g. `DirectoryInfo` / `FileInfo`-style utilities).
///
/// Implementation notes:
/// - Windows long-path quirks are handled here so callers can use consistent path semantics.
pub mod file;
pub mod file_info;
pub mod file_stream;
pub mod name_fix;
pub mod path;

pub use directory::*;
pub use directory_info::*;
pub use file::*;
pub use file_info::*;
pub use file_stream::*;
pub use name_fix::*;
pub use path::*;
