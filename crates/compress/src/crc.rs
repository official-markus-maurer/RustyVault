use crc32fast::Hasher as Crc32Hasher;

#[derive(Clone, Default)]
pub struct Crc32 {
    hasher: Crc32Hasher,
    total_bytes_read: i64,
}

impl Crc32 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.hasher = Crc32Hasher::new();
        self.total_bytes_read = 0;
    }

    pub fn slurp_block(&mut self, block: &[u8]) {
        self.total_bytes_read = self.total_bytes_read.saturating_add(block.len() as i64);
        self.hasher.update(block);
    }

    pub fn crc32_result_i32(&self) -> i32 {
        self.hasher.clone().finalize() as i32
    }

    pub fn crc32_result_u32(&self) -> u32 {
        self.hasher.clone().finalize()
    }

    pub fn crc32_result_be_bytes(&self) -> [u8; 4] {
        self.crc32_result_u32().to_be_bytes()
    }

    pub fn total_bytes_read(&self) -> i64 {
        self.total_bytes_read
    }

    pub fn calculate_digest(data: &[u8], offset: u32, size: u32) -> u32 {
        let start = offset as usize;
        let end = start.saturating_add(size as usize).min(data.len());
        let mut hasher = Crc32Hasher::new();
        hasher.update(&data[start..end]);
        hasher.finalize()
    }

    pub fn calculate_digest_be_bytes(data: &[u8], offset: u32, size: u32) -> [u8; 4] {
        Self::calculate_digest(data, offset, size).to_be_bytes()
    }

    pub fn verify_digest(digest: u32, data: &[u8], offset: u32, size: u32) -> bool {
        Self::calculate_digest(data, offset, size) == digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_known() {
        let mut crc = Crc32::new();
        crc.slurp_block(b"123456789");
        assert_eq!(crc.crc32_result_u32(), 0xCBF43926);
        assert_eq!(crc.crc32_result_be_bytes(), [0xCB, 0xF4, 0x39, 0x26]);
        assert_eq!(crc.total_bytes_read(), 9);
    }
}
