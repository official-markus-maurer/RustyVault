use crate::enums::{DatStatus, FileType, HeaderFileType, ZipStructure};

pub const TRRNTZIP_DOS_DATETIME: i64 = ((8600u32 as i64) << 16) | 48128i64;

/// In-memory Abstract Syntax Tree (AST) for parsed DAT files.
///
/// `dat_store.rs` defines the hierarchical structures (`DatHeader`, `DatNode`, `DatDir`, `DatFile`, `DatGame`)
/// that represent the raw parsed contents of an XML/CMP DAT file before it is folded into the
/// core `rv_core::DB` file tree.
///
/// Implementation notes:
/// - Parsing produces a standalone AST (`DatNode`) that is later merged into the DB tree.
/// - This separation allows parsing multiple DATs in parallel without sharing DB state.
#[derive(Debug, Clone)]
pub struct DatNode {
    pub name: String,
    pub dat_status: DatStatus,
    pub file_type: FileType,
    pub date_modified: Option<i64>,
    pub node: DatBase,
}

impl DatNode {
    pub fn new_dir(name: String, file_type: FileType) -> Self {
        DatNode {
            name,
            dat_status: DatStatus::InDatCollect,
            file_type,
            date_modified: None,
            node: DatBase::Dir(DatDir::new(file_type)),
        }
    }

    pub fn new_file(name: String, file_type: FileType) -> Self {
        DatNode {
            name,
            dat_status: DatStatus::InDatCollect,
            file_type,
            date_modified: None,
            node: DatBase::File(DatFile::new()),
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self.node, DatBase::Dir(_))
    }

    pub fn is_file(&self) -> bool {
        matches!(self.node, DatBase::File(_))
    }

    pub fn dir(&self) -> Option<&DatDir> {
        if let DatBase::Dir(ref d) = self.node {
            Some(d)
        } else {
            None
        }
    }

    pub fn dir_mut(&mut self) -> Option<&mut DatDir> {
        if let DatBase::Dir(ref mut d) = self.node {
            Some(d)
        } else {
            None
        }
    }

    pub fn file(&self) -> Option<&DatFile> {
        if let DatBase::File(ref f) = self.node {
            Some(f)
        } else {
            None
        }
    }

    pub fn file_mut(&mut self) -> Option<&mut DatFile> {
        if let DatBase::File(ref mut f) = self.node {
            Some(f)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub enum DatBase {
    Dir(DatDir),
    File(DatFile),
}

#[derive(Debug, Clone)]
pub struct DatDir {
    file_type: FileType,
    dat_struct: u8,
    pub d_game: Option<Box<DatGame>>,
    pub children: Vec<DatNode>,
    children_name_index: Vec<usize>,
}

impl DatDir {
    pub fn new(file_type: FileType) -> Self {
        Self {
            file_type,
            dat_struct: 0,
            d_game: None,
            children: Vec::new(),
            children_name_index: Vec::new(),
        }
    }

    pub fn take_children(&mut self) -> Vec<DatNode> {
        self.children_name_index.clear();
        std::mem::take(&mut self.children)
    }

    pub fn clear_children(&mut self) {
        self.children.clear();
        self.children_name_index.clear();
    }

    fn ensure_children_name_index(&mut self) {
        if self.file_type != FileType::UnSet {
            return;
        }
        if self.children_name_index.len() == self.children.len() {
            return;
        }
        self.children_name_index = (0..self.children.len()).collect();
        let dir_type = self.file_type;
        self.children_name_index.sort_by(|a, b| {
            let left = &self.children[*a];
            let right = &self.children[*b];
            let res = Self::compare_node_names(dir_type, left, right);
            res.cmp(&0)
        });
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    pub fn dat_struct(&self) -> ZipStructure {
        ZipStructure::from(self.dat_struct & 0x7f)
    }

    pub fn dat_struct_fix(&self) -> bool {
        (self.dat_struct & 0x80) == 0x80
    }

    pub fn set_dat_struct(&mut self, zip_structure: ZipStructure, fix: bool) {
        self.dat_struct = zip_structure as u8;
        if fix {
            self.dat_struct |= 0x80;
        }
    }

    pub fn add_child(&mut self, child: DatNode) {
        if self.file_type == FileType::UnSet {
            self.ensure_children_name_index();
            let insert_pos = Self::child_name_binary_search_indexed(
                self.file_type,
                &child,
                &self.children,
                &self.children_name_index,
                false,
            );
            self.children.push(child);
            let new_index = self.children.len() - 1;
            self.children_name_index.insert(insert_pos, new_index);
            return;
        }

        let insert_pos =
            Self::child_name_binary_search_direct(self.file_type, &child, &self.children, false);
        self.children.insert(insert_pos, child);
    }

    pub fn child_sorted(&self, index: usize) -> Option<&DatNode> {
        if self.file_type == FileType::UnSet {
            let child_index = *self.children_name_index.get(index)?;
            self.children.get(child_index)
        } else {
            self.children.get(index)
        }
    }

    fn child_name_binary_search_direct(
        dir_type: FileType,
        needle: &DatNode,
        children: &[DatNode],
        find_first: bool,
    ) -> usize {
        let mut bottom = 0usize;
        let mut top = children.len();
        let mut mid = 0usize;
        let mut res = -1i32;

        while bottom < top && res != 0 {
            mid = (bottom + top) / 2;
            res = Self::compare_node_names(dir_type, needle, &children[mid]);
            if res < 0 {
                top = mid;
            } else if res > 0 {
                bottom = mid + 1;
            }
        }

        if res == 0 {
            if find_first {
                while mid > 0 && Self::compare_node_names(dir_type, needle, &children[mid - 1]) == 0
                {
                    mid -= 1;
                }
            } else {
                while mid + 1 < children.len()
                    && Self::compare_node_names(dir_type, needle, &children[mid + 1]) == 0
                {
                    mid += 1;
                }
                mid += 1;
            }
        } else if res > 0 {
            mid += 1;
        }

        mid
    }

    fn child_name_binary_search_indexed(
        dir_type: FileType,
        needle: &DatNode,
        children: &[DatNode],
        index: &[usize],
        find_first: bool,
    ) -> usize {
        if index.is_empty() || children.is_empty() {
            return 0;
        }
        let mut bottom = 0usize;
        let mut top = index.len();
        let mut mid = 0usize;
        let mut res = -1i32;

        while bottom < top && res != 0 {
            mid = (bottom + top) / 2;
            let child_idx = index[mid];
            let Some(current) = children.get(child_idx) else {
                return 0;
            };
            res = Self::compare_node_names(dir_type, needle, current);
            if res < 0 {
                top = mid;
            } else if res > 0 {
                bottom = mid + 1;
            }
        }

        if res == 0 {
            if find_first {
                while mid > 0
                    && Self::compare_node_names(dir_type, needle, &children[index[mid - 1]]) == 0
                {
                    mid -= 1;
                }
            } else {
                while mid + 1 < index.len()
                    && Self::compare_node_names(dir_type, needle, &children[index[mid + 1]]) == 0
                {
                    mid += 1;
                }
                mid += 1;
            }
        } else if res > 0 {
            mid += 1;
        }

        mid
    }

    fn compare_node_names(dir_type: FileType, left: &DatNode, right: &DatNode) -> i32 {
        let res = match dir_type {
            FileType::UnSet => Self::string_compare(&left.name, &right.name),
            FileType::Dir => Self::directory_name_compare_case(&left.name, &right.name),
            FileType::Zip => Self::trrnt_zip_string_compare_case(&left.name, &right.name),
            FileType::SevenZip => Self::trrnt_7zip_string_compare(&left.name, &right.name),
            _ => Self::string_compare(&left.name, &right.name),
        };

        if res != 0 {
            return res;
        }

        match left.file_type.cmp(&right.file_type) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    fn string_compare(a: &str, b: &str) -> i32 {
        match a.cmp(b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    fn directory_name_compare(a: &str, b: &str) -> i32 {
        let la = a.to_ascii_lowercase();
        let lb = b.to_ascii_lowercase();
        Self::string_compare(&la, &lb)
    }

    fn directory_name_compare_case(a: &str, b: &str) -> i32 {
        let res = Self::directory_name_compare(a, b);
        if res != 0 {
            return res;
        }
        Self::string_compare(a, b)
    }

    fn ascii_lower(byte: u8) -> u8 {
        if byte.is_ascii_uppercase() {
            byte + 0x20
        } else {
            byte
        }
    }

    fn trrnt_zip_string_compare(a: &str, b: &str) -> i32 {
        let bytes_a = a.as_bytes();
        let bytes_b = b.as_bytes();
        let len = std::cmp::min(bytes_a.len(), bytes_b.len());

        for i in 0..len {
            let ca = Self::ascii_lower(bytes_a[i]);
            let cb = Self::ascii_lower(bytes_b[i]);

            if ca < cb {
                return -1;
            }
            if ca > cb {
                return 1;
            }
        }

        if bytes_a.len() < bytes_b.len() {
            -1
        } else if bytes_a.len() > bytes_b.len() {
            1
        } else {
            0
        }
    }

    fn trrnt_zip_string_compare_case(a: &str, b: &str) -> i32 {
        let res = Self::trrnt_zip_string_compare(a, b);
        if res != 0 {
            return res;
        }
        Self::string_compare(a, b)
    }

    fn split_filename(filename: &str) -> (&str, &str, &str) {
        let dir_index = filename.rfind('/');
        let (path, name) = if let Some(i) = dir_index {
            (&filename[..i], &filename[i + 1..])
        } else {
            ("", filename)
        };

        let ext_index = name.rfind('.');
        if let Some(i) = ext_index {
            (path, &name[..i], &name[i + 1..])
        } else {
            (path, name, "")
        }
    }

    fn trrnt_7zip_string_compare(a: &str, b: &str) -> i32 {
        let (path_a, name_a, ext_a) = Self::split_filename(a);
        let (path_b, name_b, ext_b) = Self::split_filename(b);

        let res = Self::string_compare(ext_a, ext_b);
        if res != 0 {
            return res;
        }
        let res = Self::string_compare(name_a, name_b);
        if res != 0 {
            return res;
        }
        Self::string_compare(path_a, path_b)
    }
}

impl Default for DatDir {
    fn default() -> Self {
        Self::new(FileType::Dir)
    }
}

#[cfg(test)]
mod dat_dir_tests {
    use super::*;

    #[test]
    fn datdir_unset_preserves_insertion_order_but_supports_sorted_view() {
        let mut d = DatDir::new(FileType::UnSet);
        d.add_child(DatNode::new_file("b".to_string(), FileType::File));
        d.add_child(DatNode::new_file("a".to_string(), FileType::File));

        assert_eq!(d.children[0].name, "b");
        assert_eq!(d.children[1].name, "a");

        assert_eq!(d.child_sorted(0).unwrap().name, "a");
        assert_eq!(d.child_sorted(1).unwrap().name, "b");
    }

    #[test]
    fn datdir_dir_sorts_case_insensitive_then_case_sensitive() {
        let mut d = DatDir::new(FileType::Dir);
        d.add_child(DatNode::new_dir("b".to_string(), FileType::Dir));
        d.add_child(DatNode::new_dir("A".to_string(), FileType::Dir));

        assert_eq!(d.children[0].name, "A");
        assert_eq!(d.children[1].name, "b");
    }

    #[test]
    fn datdir_zip_sorts_trrntzip_case_rules() {
        let mut d = DatDir::new(FileType::Zip);
        d.add_child(DatNode::new_file("a".to_string(), FileType::File));
        d.add_child(DatNode::new_file("A".to_string(), FileType::File));

        assert_eq!(d.children[0].name, "A");
        assert_eq!(d.children[1].name, "a");
    }

    #[test]
    fn datdir_sevenzip_sorts_by_extension_then_name_then_path() {
        let mut d = DatDir::new(FileType::SevenZip);
        d.add_child(DatNode::new_file("b.zzz".to_string(), FileType::File));
        d.add_child(DatNode::new_file("a.aaa".to_string(), FileType::File));

        assert_eq!(d.children[0].name, "a.aaa");
        assert_eq!(d.children[1].name, "b.zzz");
    }

    #[test]
    fn datdir_tiebreaks_on_child_filetype_when_names_match() {
        let mut d = DatDir::new(FileType::UnSet);
        d.add_child(DatNode::new_file("same".to_string(), FileType::File));
        d.add_child(DatNode::new_dir("same".to_string(), FileType::Dir));

        assert_eq!(d.child_sorted(0).unwrap().file_type, FileType::Dir);
        assert_eq!(d.child_sorted(1).unwrap().file_type, FileType::File);
    }
}

#[derive(Debug, Clone, Default)]
pub struct DatFile {
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub sha256: Option<Vec<u8>>,
    pub merge: Option<String>,
    pub status: Option<String>,
    pub region: Option<String>,
    pub mia: Option<String>,
    pub is_disk: bool,
    pub header_file_type: HeaderFileType,
}

impl DatFile {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct DatGame {
    pub id: Option<String>,
    pub description: Option<String>,
    pub manufacturer: Option<String>,
    pub history: Option<String>,
    pub clone_of: Option<String>,
    pub clone_of_id: Option<String>,
    pub rom_of: Option<String>,
    pub sample_of: Option<String>,
    pub source_file: Option<String>,
    pub is_bios: Option<String>,
    pub is_device: Option<String>,
    pub board: Option<String>,
    pub year: Option<String>,
    pub runnable: Option<String>,

    pub category: Vec<String>,
    pub device_ref: Vec<String>,

    pub is_emu_arc: bool,
    pub publisher: Option<String>,
    pub developer: Option<String>,
    pub genre: Option<String>,
    pub sub_genre: Option<String>,
    pub ratings: Option<String>,
    pub score: Option<String>,
    pub players: Option<String>,
    pub enabled: Option<String>,
    pub crc: Option<String>,
    pub source: Option<String>,
    pub related_to: Option<String>,

    pub game_hash: Option<Vec<u8>>,
    pub found: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DatHeader {
    pub id: Option<String>,
    pub filename: Option<String>,
    pub mame_xml: bool,
    pub name: Option<String>,
    pub type_: Option<String>, // type is reserved keyword
    pub root_dir: Option<String>,
    pub description: Option<String>,
    pub subset: Option<String>,
    pub category: Option<String>,
    pub version: Option<String>,
    pub date: Option<String>,
    pub author: Option<String>,
    pub email: Option<String>,
    pub homepage: Option<String>,
    pub url: Option<String>,
    pub comment: Option<String>,
    pub header: Option<String>,
    pub compression: Option<String>,
    pub merge_type: Option<String>,
    pub split: Option<String>,
    pub no_dump: Option<String>,
    pub dir: Option<String>,
    pub not_zipped: bool,

    pub base_dir: DatDir,
}
