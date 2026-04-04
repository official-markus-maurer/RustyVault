use std::cell::RefCell;
use std::rc::Rc;

use dat_reader::enums::FileType;
use rv_core::rv_file::{RvFile, TreeSelect};

use crate::tree_presets::{
    apply_tree_state, collect_tree_state, read_preset_file, write_preset_file,
};

#[test]
fn test_tree_presets_write_and_read_round_trip() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let a = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    a.borrow_mut().name = "A".to_string();
    a.borrow_mut().tree_checked = TreeSelect::Selected;
    a.borrow_mut().tree_expanded = true;

    let b = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    b.borrow_mut().name = "B".to_string();
    b.borrow_mut().tree_checked = TreeSelect::Locked;
    b.borrow_mut().tree_expanded = false;

    root.borrow_mut().children.push(Rc::clone(&a));
    root.borrow_mut().children.push(Rc::clone(&b));

    let entries = collect_tree_state(Rc::clone(&root));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("treeDefault1.xml");
    write_preset_file(&path.to_string_lossy(), &entries).unwrap();

    let loaded = read_preset_file(&path.to_string_lossy()).unwrap();

    a.borrow_mut().tree_checked = TreeSelect::UnSelected;
    a.borrow_mut().tree_expanded = false;
    b.borrow_mut().tree_checked = TreeSelect::UnSelected;
    b.borrow_mut().tree_expanded = true;

    apply_tree_state(Rc::clone(&root), &loaded);

    assert_eq!(a.borrow().tree_checked, TreeSelect::Selected);
    assert!(a.borrow().tree_expanded);
    assert_eq!(b.borrow().tree_checked, TreeSelect::Locked);
    assert!(!b.borrow().tree_expanded);
}
