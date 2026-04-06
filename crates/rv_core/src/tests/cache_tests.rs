use super::*;
use dat_reader::enums::FileType;
use tempfile::tempdir;

fn write_7bit_encoded_u32(out: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn write_dotnet_string(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    write_7bit_encoded_u32(out, b.len() as u32);
    out.extend_from_slice(b);
}

fn write_u32_le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_i32_le(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_u64_le(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_i64_le(out: &mut Vec<u8>, v: i64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn write_tree_row(out: &mut Vec<u8>, expanded: bool, checked: u8) {
    out.push(if expanded { 1 } else { 0 });
    out.push(checked);
}

#[allow(clippy::too_many_arguments)]
fn write_csharp_rvfile_root(
    out: &mut Vec<u8>,
    flags: u32,
    name: &str,
    file_name: &str,
    dat_status: u8,
    got_status: u8,
    children: &[Vec<u8>],
    file_status: u32,
) {
    write_u32_le(out, flags);
    write_dotnet_string(out, name);
    write_dotnet_string(out, file_name);
    write_i64_le(out, 0);
    out.push(dat_status);
    out.push(got_status);
    if flags & (1 << 14) != 0 {
        write_tree_row(out, true, 1);
    }
    if flags & (1 << 18) != 0 {
        write_i32_le(out, children.len() as i32);
        for child in children {
            out.extend_from_slice(child);
        }
    }
    write_u32_le(out, file_status);
}

#[allow(clippy::too_many_arguments)]
fn write_csharp_rvfile_child(
    out: &mut Vec<u8>,
    file_type: u8,
    flags: u32,
    name: &str,
    file_name: &str,
    dat_status: u8,
    got_status: u8,
    children: &[Vec<u8>],
    file_status: u32,
) {
    out.push(file_type);
    write_u32_le(out, flags);
    write_dotnet_string(out, name);
    write_dotnet_string(out, file_name);
    write_i64_le(out, 0);
    out.push(dat_status);
    out.push(got_status);
    if flags & (1 << 14) != 0 {
        write_tree_row(out, true, 1);
    }
    if flags & (1 << 18) != 0 {
        write_i32_le(out, children.len() as i32);
        for child in children {
            out.extend_from_slice(child);
        }
    }
    write_u32_le(out, file_status);
}

#[test]
fn test_cache_serialization_and_relinking() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    child.borrow_mut().name = "File1.zip".to_string();

    let dat = Rc::new(RefCell::new(crate::rv_dat::RvDat::new()));
    dat.borrow_mut().dat_index = 0;
    root.borrow_mut().dir_dats.push(Rc::clone(&dat));

    child.borrow_mut().set_dat_ref(Some(Rc::clone(&dat)));

    root.borrow_mut().child_add(Rc::clone(&child));

    assert_eq!(child.borrow().dat_index_for_serde, Some(0));

    // Unlink explicitly to simulate raw deserialized state
    child.borrow_mut().parent = None;
    child.borrow_mut().dat = None;

    // Relink
    Cache::relink_parents(Rc::clone(&root), None, None);

    // Verify parent link restored
    assert!(child.borrow().parent.is_some());
    let p = child.borrow().parent.as_ref().unwrap().upgrade().unwrap();
    assert_eq!(p.borrow().name, "Root");

    // Verify Dat reference restored
    assert!(child.borrow().dat.is_some());
    assert_eq!(child.borrow().dat.as_ref().unwrap().borrow().dat_index, 0);
}

#[test]
fn test_cache_relink_forces_tosort_dat_status_from_tosort_flags() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let cache_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut d = cache_dir.borrow_mut();
        d.name = "Cache".to_string();
        d.dat_status = dat_reader::enums::DatStatus::NotInDat;
        d.to_sort_type = crate::enums::ToSortDirType::TO_SORT_CACHE;
    }

    let cached_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = cached_file.borrow_mut();
        f.name = "from_cache.bin".to_string();
        f.dat_status = dat_reader::enums::DatStatus::NotInDat;
        f.got_status = dat_reader::enums::GotStatus::Got;
        f.rep_status_reset();
    }
    cache_dir.borrow_mut().child_add(Rc::clone(&cached_file));

    root.borrow_mut().child_add(Rc::clone(&cache_dir));

    cached_file.borrow_mut().parent = None;
    cache_dir.borrow_mut().parent = None;

    Cache::relink_parents(Rc::clone(&root), None, None);

    assert_eq!(
        cache_dir.borrow().dat_status(),
        dat_reader::enums::DatStatus::InToSort
    );
    assert_eq!(
        cached_file.borrow().dat_status(),
        dat_reader::enums::DatStatus::InToSort
    );
}

#[test]
fn test_cache_write_then_read_roundtrip() {
    let original_settings = crate::settings::get_settings();
    let temp = tempdir().unwrap();
    let cache_file = temp.path().join("TestCache.bin");
    crate::settings::update_settings(crate::settings::Settings {
        cache_file: cache_file.to_string_lossy().into_owned(),
        cache_save_timer_enabled: false,
        ..Default::default()
    });

    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();
    let child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    child.borrow_mut().name = "File1.bin".to_string();
    root.borrow_mut().child_add(Rc::clone(&child));

    let _ = Cache::write_cache(Rc::clone(&root));
    assert!(cache_file.exists());

    let (loaded, _) = Cache::read_cache_from_path(&cache_file)
        .unwrap_or_else(|e| panic!("cache should deserialize: {e}"));
    assert_eq!(loaded.borrow().name, "Root");
    assert_eq!(loaded.borrow().children.len(), 1);
    assert_eq!(loaded.borrow().children[0].borrow().name, "File1.bin");

    crate::settings::update_settings(original_settings);
}

#[test]
fn test_cache_can_read_csharp_cache_and_migrates_tosort_flags() {
    let temp = tempdir().unwrap();
    let cache_file = temp.path().join("RomVault3_3.Cache");

    const END: u64 = 0x15a600dda7;
    let mut bytes = Vec::new();
    write_i32_le(&mut bytes, 3);

    let child_flags = (1 << 14) as u32;
    let mut rv_child = Vec::new();
    write_csharp_rvfile_child(&mut rv_child, 1, child_flags, "RomVault", "", 0, 0, &[], 0);

    let file_flags = 0u32;
    let mut tosort_file = Vec::new();
    write_csharp_rvfile_child(&mut tosort_file, 4, file_flags, "a.bin", "", 3, 1, &[], 0);

    let tosort_flags = ((1 << 14) | (1 << 18)) as u32;
    let mut tosort_child = Vec::new();
    write_csharp_rvfile_child(
        &mut tosort_child,
        1,
        tosort_flags,
        "ToSort",
        "",
        3,
        1,
        &[tosort_file],
        (1u32 << 30) | (1u32 << 31),
    );

    let root_flags = ((1 << 14) | (1 << 18)) as u32;
    write_csharp_rvfile_root(
        &mut bytes,
        root_flags,
        "",
        "",
        0,
        0,
        &[rv_child, tosort_child],
        0,
    );
    write_u64_le(&mut bytes, END);

    std::fs::write(&cache_file, &bytes).unwrap();
    let (root, _) = Cache::read_cache_from_path(&cache_file).unwrap();
    assert_eq!(root.borrow().children.len(), 2);

    let tosort = Rc::clone(&root.borrow().children[1]);
    assert!(tosort
        .borrow()
        .to_sort_type
        .contains(crate::enums::ToSortDirType::TO_SORT_PRIMARY));
    assert!(tosort
        .borrow()
        .to_sort_type
        .contains(crate::enums::ToSortDirType::TO_SORT_CACHE));

    let bits = tosort.borrow().file_status.bits();
    assert_eq!(bits & ((1u32 << 30) | (1u32 << 31)), 0);

    assert_eq!(
        tosort.borrow().dat_status(),
        dat_reader::enums::DatStatus::InToSort
    );
    assert_eq!(
        tosort.borrow().children[0].borrow().dat_status(),
        dat_reader::enums::DatStatus::InToSort
    );
}

#[test]
fn test_decode_csharp_cache_rejects_bad_end_marker() {
    const END: u64 = 0x15a600dda7;
    let mut bytes = Vec::new();
    write_i32_le(&mut bytes, 3);
    write_csharp_rvfile_root(&mut bytes, 0, "", "", 0, 0, &[], 0);
    write_u64_le(&mut bytes, END ^ 1);

    let err = match Cache::decode_csharp_cache_from_bytes(&bytes) {
        Ok(_) => panic!("expected decode to fail"),
        Err(e) => e,
    };
    assert!(err.contains("end marker"));
}
