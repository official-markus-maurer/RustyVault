type HashDataEntry = (
    usize,
    u64,
    Option<u64>,
    Option<[u8; 4]>,
    Option<[u8; 4]>,
    Option<[u8; 20]>,
    Option<[u8; 20]>,
    Option<[u8; 16]>,
    Option<[u8; 16]>,
);

type CrcIndexMap = HashMap<(u64, [u8; 4]), Vec<usize>>;
type Sha1IndexMap = HashMap<(u64, [u8; 20]), Vec<usize>>;
type Md5IndexMap = HashMap<(u64, [u8; 16]), Vec<usize>>;
type HashIndexes = (CrcIndexMap, Sha1IndexMap, Md5IndexMap);

impl FindFixes {
    fn collect_hash_data(files: &[Rc<RefCell<RvFile>>]) -> Vec<HashDataEntry> {
        let mut hash_data = Vec::with_capacity(files.len());
        for (idx, file) in files.iter().enumerate() {
            let file_ref = file.borrow();
            let crc: Option<[u8; 4]> = file_ref.crc.as_deref().and_then(|b| b.try_into().ok());
            let alt_crc: Option<[u8; 4]> =
                file_ref.alt_crc.as_deref().and_then(|b| b.try_into().ok());
            let sha1: Option<[u8; 20]> = file_ref.sha1.as_deref().and_then(|b| b.try_into().ok());
            let alt_sha1: Option<[u8; 20]> =
                file_ref.alt_sha1.as_deref().and_then(|b| b.try_into().ok());
            let md5: Option<[u8; 16]> = file_ref.md5.as_deref().and_then(|b| b.try_into().ok());
            let alt_md5: Option<[u8; 16]> =
                file_ref.alt_md5.as_deref().and_then(|b| b.try_into().ok());
            hash_data.push((
                idx,
                file_ref.size.unwrap_or(0),
                file_ref.alt_size,
                crc,
                alt_crc,
                sha1,
                alt_sha1,
                md5,
                alt_md5,
            ));
        }
        hash_data
    }

    fn build_hash_maps(hash_data: &[HashDataEntry]) -> HashIndexes {
        let (crc_map, (sha1_map, md5_map)) = rayon::join(
            || {
                let mut map: CrcIndexMap = HashMap::with_capacity(hash_data.len() * 2);
                for (idx, size, alt_size, crc, alt_crc, _, _, _, _) in hash_data.iter() {
                    if let Some(c) = crc.as_ref() {
                        map.entry((*size, *c)).or_default().push(*idx);
                    }
                    if let Some(c) = alt_crc.as_ref() {
                        map.entry(((*alt_size).unwrap_or(*size), *c))
                            .or_default()
                            .push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: Sha1IndexMap = HashMap::with_capacity(hash_data.len() * 2);
                        for (idx, size, alt_size, _, _, sha1, alt_sha1, _, _) in hash_data.iter() {
                            if let Some(s) = sha1.as_ref() {
                                map.entry((*size, *s)).or_default().push(*idx);
                            }
                            if let Some(s) = alt_sha1.as_ref() {
                                map.entry(((*alt_size).unwrap_or(*size), *s))
                                    .or_default()
                                    .push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: Md5IndexMap = HashMap::with_capacity(hash_data.len() * 2);
                        for (idx, size, alt_size, _, _, _, _, md5, alt_md5) in hash_data.iter() {
                            if let Some(m) = md5.as_ref() {
                                map.entry((*size, *m)).or_default().push(*idx);
                            }
                            if let Some(m) = alt_md5.as_ref() {
                                map.entry(((*alt_size).unwrap_or(*size), *m))
                                    .or_default()
                                    .push(*idx);
                            }
                        }
                        map
                    },
                )
            },
        );
        (crc_map, sha1_map, md5_map)
    }
}
