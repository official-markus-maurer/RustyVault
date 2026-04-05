mod internals;
pub use internals::{extract_entry_bytes, seven_zip_dictionary_size_from_uncompressed_size};

include!("seven_zip/impl.rs");

#[cfg(test)]
#[path = "tests/seven_zip_tests.rs"]
mod tests;
