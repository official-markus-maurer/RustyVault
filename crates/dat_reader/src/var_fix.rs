const VALID_HEX: &str = "0123456789abcdef";

pub fn string_yes_no(value: &str) -> bool {
    let v = value.trim().to_ascii_lowercase();
    v == "yes" || v == "true"
}

pub fn u64_opt(value: &str) -> Option<u64> {
    let n = value.trim();
    if n.is_empty() || n == "-" {
        return None;
    }
    let lower = n.to_ascii_lowercase();
    if let Some(stripped) = lower.strip_prefix("0x") {
        return u64::from_str_radix(stripped, 16).ok();
    }
    n.parse::<u64>().ok()
}

pub fn to_lower(value: &str) -> String {
    value.to_ascii_lowercase()
}

pub fn clean_chd(value: &str) -> String {
    let mut disk = value.trim().to_string();
    if disk.to_ascii_lowercase().ends_with(".chd") && disk.len() >= 4 {
        disk.truncate(disk.len() - 4);
    }
    disk
}

pub fn clean_md5_sha1(checksum: &str, length: usize) -> Option<Vec<u8>> {
    let mut c = checksum.trim().to_ascii_lowercase();
    if c.is_empty() || c == "-" {
        return None;
    }
    if c.starts_with("0x") {
        c = c[2..].to_string();
    }
    if c.is_empty() || c == "-" {
        return None;
    }
    while c.len() < length {
        c.insert(0, '0');
    }
    for ch in c.chars() {
        VALID_HEX.find(ch)?;
    }
    hex::decode(c).ok()
}

pub fn clean_file_name(name: &str, replacement: char) -> String {
    if name.is_empty() {
        return String::new();
    }
    let mut ret = name.trim_start().trim_end_matches(['.', ' ']).to_string();
    let mut chars: Vec<char> = ret.chars().collect();
    for ch in &mut chars {
        let v = *ch as u32;
        if matches!(*ch, ':' | '*' | '?' | '<' | '>' | '|' | '"' | '\\' | '/') || v < 32 {
            *ch = replacement;
        }
    }
    ret = chars.into_iter().collect();
    ret
}

pub fn bytes_to_hex(bytes: Option<&[u8]>) -> String {
    match bytes {
        Some(b) => hex::encode(b),
        None => String::new(),
    }
}
