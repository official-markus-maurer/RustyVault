    use super::*;
    use dat_reader::enums::FileType;

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
