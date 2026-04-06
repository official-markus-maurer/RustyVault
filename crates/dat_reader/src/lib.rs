pub mod cmp_reader;
pub mod dat_clean;
pub mod dat_store;
pub mod dos_reader;
pub mod dos_writer;
pub mod enums;
pub mod json_writer;
pub mod mess_xml_reader;
pub mod rom_center_reader;
pub mod var_fix;
pub mod xml_reader;
pub mod xml_writer;

pub use cmp_reader::*;
pub use dat_clean::*;
pub use dat_store::*;
pub use dos_reader::*;
pub use dos_writer::*;
pub use enums::*;
pub use json_writer::*;
pub use mess_xml_reader::*;
pub use rom_center_reader::*;
pub use var_fix::*;
pub use xml_reader::*;
pub use xml_writer::*;

/// Main entry point for reading DAT files.
///
/// `read_dat` sniffs the file content to detect whether it is an XML DAT,
/// ClrMamePro (CMP) DAT, DOSCenter DAT, or RomCenter DAT, and routes it to
/// the appropriate highly optimized parser.
///
/// Implementation notes:
/// - Uses a borrowed-or-owned `Cow<str>` to avoid allocations when inputs are valid UTF-8.
/// - Sniffs format using lightweight signature checks, then dispatches to specialized parsers.
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
    if first_lower.contains("xml")
        || first_lower.contains("doctype")
        || first_lower.contains("datafile")
    {
        // Fast DTD strip if present
        let has_dtd = content.contains("<!DOCTYPE") || content.contains("<!doctype");
        if has_dtd {
            let mut cleaned = String::with_capacity(content.len());
            let bytes = content.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if i + 9 < bytes.len() && bytes[i..i + 9].eq_ignore_ascii_case(b"<!doctype") {
                    i += 9;
                    let mut in_subset = false;
                    while i < bytes.len() {
                        if !in_subset && bytes[i] == b'[' {
                            in_subset = true;
                            i += 1;
                            continue;
                        }
                        if in_subset
                            && bytes[i] == b']'
                            && i + 1 < bytes.len()
                            && bytes[i + 1] == b'>'
                        {
                            i += 2;
                            break;
                        }
                        if !in_subset && bytes[i] == b'>' {
                            i += 1;
                            break;
                        }
                        i += 1;
                    }
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
