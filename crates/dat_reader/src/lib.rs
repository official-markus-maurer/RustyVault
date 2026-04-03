pub mod dat_store;
pub mod enums;
pub mod xml_reader;
pub mod cmp_reader;
pub mod rom_center_reader;
pub mod dos_reader;
pub mod mess_xml_reader;
pub mod xml_writer;
pub mod json_writer;
pub mod dos_writer;
pub mod dat_clean;
pub mod var_fix;

pub use dat_store::*;
pub use enums::*;
pub use xml_reader::*;
pub use cmp_reader::*;
pub use rom_center_reader::*;
pub use dos_reader::*;
pub use mess_xml_reader::*;
pub use xml_writer::*;
pub use json_writer::*;
pub use dos_writer::*;
pub use dat_clean::*;
pub use var_fix::*;

/// Main entry point for reading DAT files.
/// 
/// `read_dat` sniffs the file content to detect whether it is an XML DAT, 
/// ClrMamePro (CMP) DAT, DOSCenter DAT, or RomCenter DAT, and routes it to 
/// the appropriate highly optimized parser.
/// 
/// Differences from C#:
/// - C# uses a stream-based parser architecture (`DatReader`) that reads files line-by-line 
///   or block-by-block.
/// - The Rust version takes advantage of zero-copy (or low-copy) full-file buffers `Cow<str>`, 
///   rapidly searching for signatures (`<!DOCTYPE`, `<datafile>`) and feeding them to specialized 
///   parsers (`quick-xml` for XML, custom iterators for CMP) to achieve massively higher throughput.
pub fn read_dat(buffer: &[u8], filename: &str) -> Result<DatHeader, String> {
    use std::borrow::Cow;
    // Avoid an unconditional allocation: borrow when UTF-8 is valid, else fallback to lossy String
    let content: Cow<str> = match std::str::from_utf8(buffer) {
        Ok(s) => Cow::Borrowed(s),
        Err(_) => Cow::Owned(String::from_utf8_lossy(buffer).into_owned()),
    };

    // Quickly detect the first non-empty trimmed line without extra allocations
    let mut first_line: &str = "";
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            first_line = trimmed;
            break;
        }
    }
    if first_line.is_empty() {
        return Err("Empty DAT file".to_string());
    }
    let first_lower = first_line.to_ascii_lowercase();

    // Fast path for XML-like signatures
    if first_lower.contains("xml") || first_lower.contains("doctype") || first_lower.contains("datafile") {
        // Fast DTD strip if present (single <!DOCTYPE ... > block)
        let has_dtd = content.contains("<!DOCTYPE") || content.contains("<!doctype");
        if has_dtd {
            let mut cleaned = String::with_capacity(content.len());
            let bytes = content.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if i + 9 < bytes.len() && bytes[i..i + 9].eq_ignore_ascii_case(b"<!doctype") {
                    // Skip until first '>' after the doctype start
                    i += 9;
                    while i < bytes.len() && bytes[i] != b'>' {
                        i += 1;
                    }
                    i += 1;
                    continue;
                }
                cleaned.push(bytes[i] as char);
                i += 1;
            }
            if first_lower.contains("softwarelist") || cleaned.contains("<softwarelist") {
                return read_mess_xml_dat(&cleaned, filename);
            } else {
                return read_xml_dat(&cleaned, filename);
            }
        }
        if first_lower.contains("softwarelist") || content.contains("<softwarelist") {
            return read_mess_xml_dat(&content, filename);
        } else {
            return read_xml_dat(&content, filename);
        }
    }

    // Non-XML formats
    if first_lower.contains("clrmamepro")
        || first_lower.contains("clrmame")
        || first_lower.contains("romvault")
        || first_lower.contains("game")
        || first_lower.contains("machine")
    {
        return read_cmp_dat(&content, filename);
    } else if first_lower.contains("doscenter") {
        return read_dos_dat(&content, filename);
    } else if first_lower.contains("[credits]") {
        return read_rom_center_dat(&content, filename);
    }

    // Fallbacks
    if let Ok(dat) = read_xml_dat(&content, filename) {
        Ok(dat)
    } else if let Ok(dat) = read_cmp_dat(&content, filename) {
        Ok(dat)
    } else {
        if content.contains("<?xml") || content.contains("<datafile>") {
            if let Ok(dat) = read_xml_dat(&content, filename) {
                return Ok(dat);
            }
        }
        if content.contains("clrmamepro (") || content.contains("romvault (") {
            if let Ok(dat) = read_cmp_dat(&content, filename) {
                return Ok(dat);
            }
        }
        Err("Unsupported DAT format".to_string())
    }
}
