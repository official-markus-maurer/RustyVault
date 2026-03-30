#[derive(Debug, Clone)]
pub struct ZippedFile {
    pub index: i32,
    pub name: String,
    pub size: u64,
    pub crc: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub is_dir: bool,
}

impl ZippedFile {
    pub fn new() -> Self {
        Self {
            index: -1,
            name: String::new(),
            size: 0,
            crc: None,
            sha1: None,
            is_dir: false,
        }
    }
}
