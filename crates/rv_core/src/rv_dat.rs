#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DatUpdateStatus {
    Delete,
    Correct,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
    pub struct DatFlags: u8 {
        const AUTO_ADDED_DIRECTORY = 1;
        const MULTI_DAT_OVERRIDE = 2;
        const MULTI_DATS_IN_DIRECTORY = 4;
        const USE_DESCRIPTION_AS_DIR_NAME = 8;
        const SINGLE_ARCHIVE = 16;
        const USE_ID_FOR_NAME = 32;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DatData {
    Id = 0,
    DatName = 1,
    DatRootFullName = 2,
    RootDir = 3,
    Description = 4,
    Category = 5,
    Version = 6,
    Date = 7,
    Author = 8,
    Email = 9,
    HomePage = 10,
    Url = 11,
    FileType = 12,
    MergeType = 13,
    SuperDat = 14,
    DirSetup = 15,
    Header = 16,
    SubDirType = 17,
    Compression = 18,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatMetaData {
    pub id: DatData,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RvDat {
    pub game_meta_data: Vec<DatMetaData>,
    pub dat_index: i32,
    pub status: DatUpdateStatus,
    pub time_stamp: i64,
    pub dat_flags: DatFlags,
}

impl RvDat {
    pub fn new() -> Self {
        Self {
            game_meta_data: Vec::new(),
            dat_index: -1,
            status: DatUpdateStatus::Correct,
            time_stamp: 0,
            dat_flags: DatFlags::empty(),
        }
    }

    pub fn set_data(&mut self, id: DatData, value: Option<String>) {
        if let Some(val) = value {
            if val.trim().is_empty() {
                return;
            }
            if let Some(meta) = self.game_meta_data.iter_mut().find(|m| m.id == id) {
                meta.value = val;
            } else {
                self.game_meta_data.push(DatMetaData { id, value: val });
            }
        }
    }

    pub fn get_data(&self, id: DatData) -> Option<String> {
        self.game_meta_data.iter().find(|m| m.id == id).map(|m| m.value.clone())
    }

    pub fn flag(&self, flag: DatFlags) -> bool {
        self.dat_flags.contains(flag)
    }
}
