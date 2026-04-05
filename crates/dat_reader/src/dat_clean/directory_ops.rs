impl DatClean {
    pub fn directory_flatten(d_dir: &mut DatDir) {
        let mut list = Vec::new();
        Self::directory_flat(d_dir, &mut list, "");
        d_dir.children.clear();
        d_dir.children.extend(list);
    }

    fn directory_flat(d_dir: &mut DatDir, new_dir: &mut Vec<DatNode>, sub_dir: &str) {
        let mut children = Vec::new();
        children.append(&mut d_dir.children);
        for mut node in children {
            let is_game = node.dir().is_some_and(|d| d.d_game.is_some());
            if is_game {
                node.name = if sub_dir.is_empty() {
                    node.name
                } else {
                    format!("{}\\{}", sub_dir, node.name)
                };
                new_dir.push(node);
            } else {
                let next_name = if sub_dir.is_empty() {
                    node.name.clone()
                } else {
                    format!("{}\\{}", sub_dir, node.name)
                };
                if let Some(child_dir) = node.dir_mut() {
                    Self::directory_flat(child_dir, new_dir, &next_name);
                }
            }
        }
    }

    pub fn archive_directory_flatten(d_dir: &mut DatDir) {
        if d_dir.d_game.is_some() {
            let mut list = Vec::new();
            Self::archive_flat(d_dir, &mut list, "");
            d_dir.children.clear();
            d_dir.children.extend(list);
            return;
        }
        for node in &mut d_dir.children {
            if let Some(dat_dir) = node.dir_mut() {
                Self::archive_directory_flatten(dat_dir);
            }
        }
    }

    fn archive_flat(d_dir: &DatDir, new_dir: &mut Vec<DatNode>, sub_dir: &str) {
        for node in &d_dir.children {
            let this_name = if sub_dir.is_empty() {
                node.name.to_string()
            } else {
                format!("{}/{}", sub_dir, node.name)
            };
            if node.file().is_some() {
                let mut new_node = node.clone();
                new_node.name = this_name;
                new_dir.push(new_node);
                continue;
            }
            let Some(child_dir) = node.dir() else {
                continue;
            };
            let mut dir_marker = DatNode::new_file(format!("{}/", this_name), FileType::UnSet);
            if let Some(f) = dir_marker.file_mut() {
                f.size = Some(0);
                f.crc = Some(vec![0, 0, 0, 0]);
            }
            new_dir.push(dir_marker);
            Self::archive_flat(child_dir, new_dir, &this_name);
        }
    }

    pub fn directory_sort(d_dir: &mut DatDir) {
        d_dir
            .children
            .sort_by(|a, b| Self::alphanum_cmp(&a.name, &b.name).cmp(&0));
        for node in &mut d_dir.children {
            if let Some(dat_dir) = node.dir_mut() {
                Self::directory_sort(dat_dir);
            }
        }
    }

    pub fn directory_expand(d_dir: &mut DatDir) {
        loop {
            let found = d_dir
                .children
                .iter()
                .any(|n| Self::check_dir(n.file_type) && n.name.contains('/'));
            if !found {
                break;
            }

            let mut old = Vec::new();
            old.append(&mut d_dir.children);
            for mut node in old {
                if Self::check_dir(node.file_type) && node.name.contains('/') {
                    let split = node.name.find('/').unwrap_or(0);
                    let part0 = node.name[..split].to_string();
                    let part1 = node.name[split + 1..].to_string();
                    node.name = part1;

                    let dir_index = d_dir
                        .children
                        .iter()
                        .position(|c| c.is_dir() && c.name == part0);
                    let dir_node = if let Some(i) = dir_index {
                        &mut d_dir.children[i]
                    } else {
                        d_dir
                            .children
                            .push(DatNode::new_dir(part0.clone(), FileType::Dir));
                        d_dir.children.last_mut().unwrap()
                    };
                    if !node.name.is_empty() {
                        dir_node.dir_mut().unwrap().add_child(node);
                    }
                } else {
                    d_dir.children.push(node);
                }
            }
        }

        for node in &mut d_dir.children {
            if let Some(dat_dir) = node.dir_mut() {
                Self::directory_expand(dat_dir);
            }
        }
    }
}
