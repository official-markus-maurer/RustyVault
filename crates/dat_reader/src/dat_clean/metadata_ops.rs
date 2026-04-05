impl DatClean {
    pub fn remove_device_ref(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if let Some(ddir) = node.dir_mut() {
                if let Some(game) = ddir.d_game.as_mut() {
                    game.device_ref.clear();
                }
                Self::remove_device_ref(ddir);
            }
        }
    }

    pub fn check_deduped(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if let Some(file) = node.file_mut() {
                if file
                    .status
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case("deduped"))
                {
                    node.dat_status = DatStatus::InDatMerged;
                }
            }
            if let Some(ddir) = node.dir_mut() {
                Self::check_deduped(ddir);
            }
        }
    }

    pub fn add_category(t_dat: &mut DatDir, cat_order: &[String]) {
        for node in &mut t_dat.children {
            let Some(m_game) = node.dir_mut() else {
                continue;
            };
            if m_game.d_game.is_none() {
                Self::add_category(m_game, cat_order);
                continue;
            }
            if let Some(cat) = Self::find_category(m_game, cat_order) {
                node.name = format!("{}/{}", cat, node.name);
            }
        }
    }

    pub fn find_category(m_game: &DatDir, cat_order: &[String]) -> Option<String> {
        let game = m_game.d_game.as_ref()?;
        if game.category.is_empty() {
            return None;
        }
        if game.category.len() == 1 {
            let c = game.category[0].trim();
            if !c.is_empty() {
                return Some(c.to_string());
            }
            return None;
        }

        let mut best = None::<usize>;
        for cat in &game.category {
            let c = cat.trim();
            if c.is_empty() {
                continue;
            }
            for (i, order) in cat_order.iter().enumerate() {
                if order.eq_ignore_ascii_case(c) {
                    if best.is_none_or(|b| i < b) {
                        best = Some(i);
                    }
                    break;
                }
            }
        }
        best.map(|i| cat_order[i].clone())
    }
}
