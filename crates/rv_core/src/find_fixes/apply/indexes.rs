type HashDataEntry = (
    usize,
    u64,
    Option<u64>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
);

type HashIndexMap = HashMap<(u64, Vec<u8>), Vec<usize>>;
type HashIndexes = (HashIndexMap, HashIndexMap, HashIndexMap);

impl FindFixes {
    fn collect_hash_data(files: &[Rc<RefCell<RvFile>>]) -> Vec<HashDataEntry> {
        let mut hash_data = Vec::with_capacity(files.len());
        for (idx, file) in files.iter().enumerate() {
            let file_ref = file.borrow();
            hash_data.push((
                idx,
                file_ref.size.unwrap_or(0),
                file_ref.alt_size,
                file_ref.crc.clone(),
                file_ref.alt_crc.clone(),
                file_ref.sha1.clone(),
                file_ref.alt_sha1.clone(),
                file_ref.md5.clone(),
                file_ref.alt_md5.clone(),
            ));
        }
        hash_data
    }

    fn build_hash_maps(hash_data: &[HashDataEntry]) -> HashIndexes {
        let (crc_map, (sha1_map, md5_map)) = rayon::join(
            || {
                let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                for (idx, size, alt_size, crc, alt_crc, _, _, _, _) in hash_data.iter() {
                    if let Some(c) = crc.as_ref() {
                        map.entry((*size, c.clone())).or_default().push(*idx);
                    }
                    if let Some(c) = alt_crc.as_ref() {
                        map.entry(((*alt_size).unwrap_or(*size), c.clone()))
                            .or_default()
                            .push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, sha1, alt_sha1, _, _) in hash_data.iter() {
                            if let Some(s) = sha1.as_ref() {
                                map.entry((*size, s.clone())).or_default().push(*idx);
                            }
                            if let Some(s) = alt_sha1.as_ref() {
                                map.entry(((*alt_size).unwrap_or(*size), s.clone()))
                                    .or_default()
                                    .push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, alt_size, _, _, _, _, md5, alt_md5) in hash_data.iter() {
                            if let Some(m) = md5.as_ref() {
                                map.entry((*size, m.clone())).or_default().push(*idx);
                            }
                            if let Some(m) = alt_md5.as_ref() {
                                map.entry(((*alt_size).unwrap_or(*size), m.clone()))
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
