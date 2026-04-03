use std::io::{Read, Write};

pub fn write_byte_array<W: Write>(w: &mut W, b: &[u8]) -> std::io::Result<()> {
    let len: u8 = b
        .len()
        .try_into()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "byte array too long"))?;
    w.write_all(&[len])?;
    w.write_all(b)?;
    Ok(())
}

pub fn read_byte_array<R: Read>(r: &mut R) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 1];
    r.read_exact(&mut len)?;
    let mut b = vec![0u8; len[0] as usize];
    r.read_exact(&mut b)?;
    Ok(b)
}

pub fn copy_bytes(b: Option<&[u8]>) -> Option<Vec<u8>> {
    Some(b?.to_vec())
}

pub fn copy_bytes_range(b: Option<&[u8]>, index: usize, count: usize) -> Option<Vec<u8>> {
    let b = b?;
    if index > b.len() {
        return None;
    }
    let end = index.saturating_add(count).min(b.len());
    Some(b[index..end].to_vec())
}

pub fn b_compare(b1: Option<&[u8]>, b2: Option<&[u8]>) -> bool {
    match (b1, b2) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

pub fn e_compare(b1: Option<&[u8]>, b2: Option<&[u8]>) -> bool {
    match (b1, b2) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

pub fn i_compare(b1: Option<&[u8]>, b2: Option<&[u8]>) -> i32 {
    let a = b1.unwrap_or(&[]);
    let b = b2.unwrap_or(&[]);
    let mut p = 0usize;
    loop {
        if a.len() == p {
            return if b.len() == p { 0 } else { -1 };
        }
        if b.len() == p {
            return 1;
        }
        if a[p] < b[p] {
            return -1;
        }
        if a[p] > b[p] {
            return 1;
        }
        p += 1;
    }
}

pub fn to_hex_string(b: Option<&[u8]>) -> String {
    let Some(b) = b else { return String::new(); };
    let mut out = String::with_capacity(b.len() * 2);
    for v in b {
        out.push_str(&format!("{:02x}", v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_null_is_empty() {
        assert_eq!(to_hex_string(None), "");
        assert_eq!(to_hex_string(Some(&[0xAB, 0x00, 0x01])), "ab0001");
    }

    #[test]
    fn compare_semantics() {
        assert!(!b_compare(None, Some(&[1])));
        assert!(e_compare(None, Some(&[1])));
        assert_eq!(i_compare(None, Some(&[1])), -1);
        assert_eq!(i_compare(Some(&[1]), None), 1);
        assert_eq!(i_compare(Some(&[1, 2]), Some(&[1, 3])), -1);
    }
}
