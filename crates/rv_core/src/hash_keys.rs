pub type CrcKey = (u64, [u8; 4]);
pub type Sha1Key = (u64, [u8; 20]);
pub type Md5Key = (u64, [u8; 16]);

pub fn crc_key(size: u64, bytes: &[u8]) -> Option<CrcKey> {
    let arr: [u8; 4] = bytes.try_into().ok()?;
    Some((size, arr))
}

pub fn sha1_key(size: u64, bytes: &[u8]) -> Option<Sha1Key> {
    let arr: [u8; 20] = bytes.try_into().ok()?;
    Some((size, arr))
}

pub fn md5_key(size: u64, bytes: &[u8]) -> Option<Md5Key> {
    let arr: [u8; 16] = bytes.try_into().ok()?;
    Some((size, arr))
}
