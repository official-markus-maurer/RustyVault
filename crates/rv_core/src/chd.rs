use std::io::Read;

pub struct ChdHeaderInfo {
    pub version: u32,
    pub sha1: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub requires_parent: bool,
}

fn read_u32_be(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn is_all_zero(b: &[u8]) -> bool {
    b.iter().all(|v| *v == 0)
}

pub fn parse_chd_header_from_bytes(buf: &[u8]) -> Option<ChdHeaderInfo> {
    const MAGIC: &[u8; 8] = b"MComprHD";
    const HEADER_LENGTHS: [u32; 6] = [0, 76, 80, 120, 108, 124];

    if buf.len() < 16 {
        return None;
    }
    if &buf[..8] != MAGIC {
        return None;
    }

    let length = read_u32_be(&buf[8..12]);
    let version = read_u32_be(&buf[12..16]);
    if version as usize >= HEADER_LENGTHS.len() {
        return None;
    }
    if HEADER_LENGTHS[version as usize] != length {
        return None;
    }
    if buf.len() < length as usize {
        return None;
    }

    let mut md5: Option<Vec<u8>> = None;
    let mut rawsha1: Option<Vec<u8>> = None;
    let mut sha1: Option<Vec<u8>> = None;
    let mut parentmd5: Option<&[u8]> = None;
    let mut parentsha1: Option<&[u8]> = None;

    match version {
        1 | 2 => {
            md5 = Some(buf[44..60].to_vec());
            parentmd5 = Some(&buf[60..76]);
        }
        3 => {
            md5 = Some(buf[44..60].to_vec());
            parentmd5 = Some(&buf[60..76]);
            rawsha1 = Some(buf[80..100].to_vec());
            parentsha1 = Some(&buf[100..120]);
        }
        4 => {
            sha1 = Some(buf[48..68].to_vec());
            parentsha1 = Some(&buf[68..88]);
            rawsha1 = Some(buf[88..108].to_vec());
        }
        5 => {
            rawsha1 = Some(buf[64..84].to_vec());
            sha1 = Some(buf[84..104].to_vec());
            parentsha1 = Some(&buf[104..124]);
        }
        _ => return None,
    }

    let requires_parent =
        parentmd5.is_some_and(|p| !is_all_zero(p)) || parentsha1.is_some_and(|p| !is_all_zero(p));
    let effective_sha1 = sha1.or(rawsha1);

    Some(ChdHeaderInfo {
        version,
        sha1: effective_sha1,
        md5,
        requires_parent,
    })
}

pub fn parse_chd_header_from_reader<R: Read>(mut r: R) -> Option<ChdHeaderInfo> {
    let mut header = [0u8; 124];
    if r.read_exact(&mut header[..16]).is_err() {
        return None;
    }
    if &header[..8] != b"MComprHD" {
        return None;
    }
    let length = read_u32_be(&header[8..12]) as usize;
    if length > header.len() || length < 16 {
        return None;
    }
    if r.read_exact(&mut header[16..length]).is_err() {
        return None;
    }
    parse_chd_header_from_bytes(&header[..length])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chd_v5_header_parse() {
        let mut b = vec![0u8; 124];
        b[..8].copy_from_slice(b"MComprHD");
        b[8..12].copy_from_slice(&(124u32.to_be_bytes()));
        b[12..16].copy_from_slice(&(5u32.to_be_bytes()));
        b[84..104].copy_from_slice(&[0x11u8; 20]);
        let info = parse_chd_header_from_bytes(&b).unwrap();
        assert_eq!(info.version, 5);
        assert_eq!(info.sha1.as_ref().unwrap().len(), 20);
        assert_eq!(info.sha1.as_ref().unwrap()[0], 0x11);
        assert!(info.md5.is_none());
        assert!(!info.requires_parent);
    }

    #[test]
    fn chd_v4_header_parse_prefers_stored_sha1_and_detects_parent() {
        let mut b = vec![0u8; 108];
        b[..8].copy_from_slice(b"MComprHD");
        b[8..12].copy_from_slice(&(108u32.to_be_bytes()));
        b[12..16].copy_from_slice(&(4u32.to_be_bytes()));

        b[48..68].copy_from_slice(&[0x22u8; 20]);
        b[68..88].copy_from_slice(&[0x33u8; 20]);
        b[88..108].copy_from_slice(&[0x44u8; 20]);

        let info = parse_chd_header_from_bytes(&b).unwrap();
        assert_eq!(info.version, 4);
        assert_eq!(info.sha1.as_ref().unwrap(), &[0x22u8; 20]);
        assert!(info.md5.is_none());
        assert!(info.requires_parent);
    }

    #[test]
    fn chd_v3_header_parse_uses_rawsha1_and_detects_parent() {
        let mut b = vec![0u8; 120];
        b[..8].copy_from_slice(b"MComprHD");
        b[8..12].copy_from_slice(&(120u32.to_be_bytes()));
        b[12..16].copy_from_slice(&(3u32.to_be_bytes()));

        b[44..60].copy_from_slice(&[0x55u8; 16]);
        b[60..76].copy_from_slice(&[0x66u8; 16]);
        b[80..100].copy_from_slice(&[0x77u8; 20]);
        b[100..120].copy_from_slice(&[0x88u8; 20]);

        let info = parse_chd_header_from_bytes(&b).unwrap();
        assert_eq!(info.version, 3);
        assert_eq!(info.md5.as_ref().unwrap(), &[0x55u8; 16]);
        assert_eq!(info.sha1.as_ref().unwrap(), &[0x77u8; 20]);
        assert!(info.requires_parent);
    }

    #[test]
    fn chd_invalid_magic() {
        let b = vec![0u8; 124];
        assert!(parse_chd_header_from_bytes(&b).is_none());
    }
}
