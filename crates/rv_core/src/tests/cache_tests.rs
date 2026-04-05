use super::*;
use dat_reader::enums::FileType;
use tempfile::tempdir;

#[test]
fn test_cache_serialization_and_relinking() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    child.borrow_mut().name = "File1.zip".to_string();

    let dat = Rc::new(RefCell::new(crate::rv_dat::RvDat::new()));
    dat.borrow_mut().dat_index = 0;
    root.borrow_mut().dir_dats.push(Rc::clone(&dat));

    child.borrow_mut().dat = Some(Rc::clone(&dat));

    root.borrow_mut().child_add(Rc::clone(&child));

    // Prepare for serialization
    Cache::prepare_for_serialize(Rc::clone(&root));
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

    Cache::write_cache(Rc::clone(&root));
    assert!(cache_file.exists());

    let (loaded, _) = Cache::read_cache_from_path(&cache_file)
        .unwrap_or_else(|e| panic!("cache should deserialize: {e}"));
    assert_eq!(loaded.borrow().name, "Root");
    assert_eq!(loaded.borrow().children.len(), 1);
    assert_eq!(loaded.borrow().children[0].borrow().name, "File1.bin");

    crate::settings::update_settings(original_settings);
}
