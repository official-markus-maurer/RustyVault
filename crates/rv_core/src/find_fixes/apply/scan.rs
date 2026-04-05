impl FindFixes {
    /// Recursively scans the tree to pair `Missing` files with unassigned `Got` files.
    pub fn scan_files(root: Rc<RefCell<RvFile>>) {
        info!("Starting FindFixes pass...");
        Self::reset_status(Rc::clone(&root));

        let mut all_dat_files = Vec::new();
        Self::get_all_dat_files(Rc::clone(&root), &mut all_dat_files);
        Self::hydrate_physical_dat_files(&all_dat_files);

        let mut files_got = Vec::new();
        let mut files_missing = Vec::new();
        Self::get_selected_files(Rc::clone(&root), &mut files_got, &mut files_missing);
        let mut all_got_files = Vec::new();
        Self::get_all_got_files(Rc::clone(&root), &mut all_got_files);

        info!(
            "FindFixes: Collected {} Got files and {} Missing files.",
            files_got.len(),
            files_missing.len()
        );

        let hash_data = Self::collect_hash_data(&files_got);
        let (crc_map, sha1_map, md5_map) = Self::build_hash_maps(&hash_data);

        let all_got_hash_data = Self::collect_hash_data(&all_got_files);
        let (all_got_crc_map, all_got_sha1_map, all_got_md5_map) =
            Self::build_hash_maps(&all_got_hash_data);

        let missing_hash_data = Self::collect_hash_data(&files_missing);
        let (missing_crc_map, missing_sha1_map, missing_md5_map) =
            Self::build_hash_maps(&missing_hash_data);

        let mut used_got_indices = HashSet::new();

        for missing in &files_missing {
            let mut missing_ref = missing.borrow_mut();
            let size = missing_ref.size.unwrap_or(0);

            if matches!(
                missing_ref.dat_status(),
                DatStatus::InDatMerged | DatStatus::InDatNoDump
            ) {
                missing_ref.set_rep_status(RepStatus::NotCollected);
                missing_ref.cached_stats = None;
                continue;
            }

            let mut found_got_idx = None;
            let mut crc_candidates = Vec::new();
            let mut crc_seen = HashSet::new();

            if let Some(ref crc) = missing_ref.crc {
                if let Some(got_list) = crc_map.get(&(size, crc.clone())) {
                    Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                }
            }
            if let Some(ref crc) = missing_ref.crc {
                let alt_size = missing_ref.alt_size.unwrap_or(size);
                if alt_size != size {
                    if let Some(got_list) = crc_map.get(&(alt_size, crc.clone())) {
                        Self::extend_unique_got_candidates(
                            &mut crc_candidates,
                            got_list,
                            &mut crc_seen,
                        );
                    }
                }
            }
            if let Some(ref alt_crc) = missing_ref.alt_crc {
                let alt_size = missing_ref.alt_size.unwrap_or(size);
                if let Some(got_list) = crc_map.get(&(alt_size, alt_crc.clone())) {
                    Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                }
            }
            if !crc_candidates.is_empty() {
                found_got_idx = Self::preferred_got_idx(&crc_candidates, &files_got, &used_got_indices);
            }

            if found_got_idx.is_none() {
                let mut sha1_candidates = Vec::new();
                let mut sha1_seen = HashSet::new();
                if let Some(ref sha1) = missing_ref.sha1 {
                    if let Some(got_list) = sha1_map.get(&(size, sha1.clone())) {
                        Self::extend_unique_got_candidates(
                            &mut sha1_candidates,
                            got_list,
                            &mut sha1_seen,
                        );
                    }
                }
                if let Some(ref sha1) = missing_ref.sha1 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if alt_size != size {
                        if let Some(got_list) = sha1_map.get(&(alt_size, sha1.clone())) {
                            Self::extend_unique_got_candidates(
                                &mut sha1_candidates,
                                got_list,
                                &mut sha1_seen,
                            );
                        }
                    }
                }
                if let Some(ref alt_sha1) = missing_ref.alt_sha1 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if let Some(got_list) = sha1_map.get(&(alt_size, alt_sha1.clone())) {
                        Self::extend_unique_got_candidates(
                            &mut sha1_candidates,
                            got_list,
                            &mut sha1_seen,
                        );
                    }
                }
                if !sha1_candidates.is_empty() {
                    found_got_idx =
                        Self::preferred_got_idx(&sha1_candidates, &files_got, &used_got_indices);
                }
            }

            if found_got_idx.is_none() {
                let mut md5_candidates = Vec::new();
                let mut md5_seen = HashSet::new();
                if let Some(ref md5) = missing_ref.md5 {
                    if let Some(got_list) = md5_map.get(&(size, md5.clone())) {
                        Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                    }
                }
                if let Some(ref md5) = missing_ref.md5 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if alt_size != size {
                        if let Some(got_list) = md5_map.get(&(alt_size, md5.clone())) {
                            Self::extend_unique_got_candidates(
                                &mut md5_candidates,
                                got_list,
                                &mut md5_seen,
                            );
                        }
                    }
                }
                if let Some(ref alt_md5) = missing_ref.alt_md5 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if let Some(got_list) = md5_map.get(&(alt_size, alt_md5.clone())) {
                        Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                    }
                }
                if !md5_candidates.is_empty() {
                    found_got_idx =
                        Self::preferred_got_idx(&md5_candidates, &files_got, &used_got_indices);
                }
            }

            let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
            if let Some(got_idx) = found_got_idx {
                let got = &files_got[got_idx];
                let is_corrupt = got.borrow().got_status() == GotStatus::Corrupt;

                trace!(
                    "Found fix match: missing '{}' mapped to got file.",
                    missing_ref.name
                );

                missing_ref.set_rep_status(if is_corrupt {
                    RepStatus::CorruptCanBeFixed
                } else if is_mia {
                    RepStatus::CanBeFixedMIA
                } else {
                    RepStatus::CanBeFixed
                });

                let mut got_mut = got.borrow_mut();
                let current_rep = got_mut.rep_status();
                if got_mut.got_status() != GotStatus::Corrupt
                    && (current_rep == RepStatus::UnScanned
                        || current_rep == RepStatus::InToSort
                        || current_rep == RepStatus::MoveToSort
                        || current_rep == RepStatus::Unknown
                        || current_rep == RepStatus::Deleted
                        || current_rep == RepStatus::UnNeeded)
                {
                    got_mut.set_rep_status(RepStatus::NeededForFix);
                }
                if Self::source_is_consumable(&got_mut) {
                    used_got_indices.insert(got_idx);
                }

                missing_ref.cached_stats = None;
                got_mut.cached_stats = None;
            } else {
                trace!("No fix found for missing file: {}", missing_ref.name);
                let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
                missing_ref.set_rep_status(if is_mia {
                    RepStatus::MissingMIA
                } else {
                    RepStatus::Missing
                });
                missing_ref.cached_stats = None;
            }
        }

        for (idx, got) in files_got.iter().enumerate() {
            let (got_status, rep_status, dat_status) = {
                let got_ref = got.borrow();
                (got_ref.got_status(), got_ref.rep_status(), got_ref.dat_status())
            };
            if got_status == GotStatus::Corrupt {
                let merged_cleanup_status = if matches!(
                    dat_status,
                    DatStatus::InDatMerged | DatStatus::InDatNoDump
                ) {
                    Some(Self::merged_cleanup_status(idx, &files_got))
                } else {
                    None
                };
                let mut got_ref = got.borrow_mut();
                if rep_status == RepStatus::NeededForFix {
                } else if dat_status == DatStatus::InDatCollect {
                    got_ref.set_rep_status(RepStatus::MoveToCorrupt);
                    got_ref.cached_stats = None;
                } else if matches!(
                    dat_status,
                    DatStatus::InDatMerged | DatStatus::InDatNoDump
                ) {
                    got_ref.set_rep_status(merged_cleanup_status.unwrap());
                    got_ref.cached_stats = None;
                } else {
                    got_ref.set_rep_status(RepStatus::Delete);
                    got_ref.cached_stats = None;
                }
            }
        }

        for (idx, got) in files_got.iter().enumerate() {
            let (current_rep_status, dat_status) = {
                let got_ref = got.borrow();
                (got_ref.rep_status(), got_ref.dat_status())
            };

            if current_rep_status == RepStatus::NeededForFix
                || current_rep_status == RepStatus::Correct
                || current_rep_status == RepStatus::Delete
                || current_rep_status == RepStatus::MoveToCorrupt
            {
                continue;
            }

            let merged_cleanup_status = if matches!(
                dat_status,
                DatStatus::InDatMerged | DatStatus::InDatNoDump
            ) {
                Some(Self::merged_cleanup_status(idx, &files_got))
            } else {
                None
            };

            let should_delete_tosort = if dat_status == DatStatus::InToSort {
                let got_ref = got.borrow();
                Self::has_redundant_romroot_match(
                    Rc::clone(got),
                    &got_ref,
                    &all_got_files,
                    &all_got_crc_map,
                    &all_got_sha1_map,
                    &all_got_md5_map,
                ) || Self::has_redundant_physical_dat_match(Rc::clone(got), &got_ref, &all_dat_files)
                    || Self::has_pending_fix_target_match(
                        &got_ref,
                        &files_missing,
                        &missing_crc_map,
                        &missing_sha1_map,
                        &missing_md5_map,
                    )
            } else {
                false
            };

            let should_delete_notindat = if dat_status == DatStatus::NotInDat {
                let got_ref = got.borrow();
                Self::has_redundant_collect_hash_match(
                    Rc::clone(got),
                    &got_ref,
                    &all_got_files,
                    &all_got_crc_map,
                    &all_got_sha1_map,
                    &all_got_md5_map,
                ) || Self::has_redundant_physical_collect_match(Rc::clone(got), &got_ref, &all_dat_files)
            } else {
                false
            };

            let mut got_ref = got.borrow_mut();

            if dat_status == DatStatus::InDatCollect {
                got_ref.set_rep_status(RepStatus::Correct);
                got_ref.cached_stats = None;
            } else if matches!(
                dat_status,
                DatStatus::InDatMerged | DatStatus::InDatNoDump
            ) {
                got_ref.set_rep_status(merged_cleanup_status.unwrap());
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::InDatMIA {
                got_ref.set_rep_status(RepStatus::CorrectMIA);
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::InToSort {
                if should_delete_tosort {
                    got_ref.set_rep_status(RepStatus::Delete);
                } else {
                    got_ref.set_rep_status(RepStatus::InToSort);
                }
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::NotInDat {
                if should_delete_notindat {
                    got_ref.set_rep_status(RepStatus::Delete);
                } else {
                    got_ref.set_rep_status(RepStatus::MoveToSort);
                }
                got_ref.cached_stats = None;
            }
        }

        crate::clean_partial::apply_complete_only(Rc::clone(&root));
        Self::apply_without_reset(Rc::clone(&root));
    }

    fn apply_without_reset(root: Rc<RefCell<RvFile>>) {
        let mut all_dat_files = Vec::new();
        Self::get_all_dat_files(Rc::clone(&root), &mut all_dat_files);
        Self::hydrate_physical_dat_files(&all_dat_files);

        let mut files_got = Vec::new();
        let mut files_missing = Vec::new();
        Self::get_selected_files(Rc::clone(&root), &mut files_got, &mut files_missing);
        let mut all_got_files = Vec::new();
        Self::get_all_got_files(Rc::clone(&root), &mut all_got_files);

        let hash_data = Self::collect_hash_data(&files_got);
        let (crc_map, sha1_map, md5_map) = Self::build_hash_maps(&hash_data);

        let all_got_hash_data = Self::collect_hash_data(&all_got_files);
        let (all_got_crc_map, all_got_sha1_map, all_got_md5_map) =
            Self::build_hash_maps(&all_got_hash_data);

        let missing_hash_data = Self::collect_hash_data(&files_missing);
        let (missing_crc_map, missing_sha1_map, missing_md5_map) =
            Self::build_hash_maps(&missing_hash_data);

        let mut used_got_indices = HashSet::new();

        for missing in &files_missing {
            let mut missing_ref = missing.borrow_mut();
            let size = missing_ref.size.unwrap_or(0);
            if matches!(
                missing_ref.dat_status(),
                DatStatus::InDatMerged | DatStatus::InDatNoDump
            ) {
                missing_ref.set_rep_status(RepStatus::NotCollected);
                missing_ref.cached_stats = None;
                continue;
            }

            let mut found_got_idx = None;
            let mut crc_candidates = Vec::new();
            let mut crc_seen = HashSet::new();
            if let Some(ref crc) = missing_ref.crc {
                if let Some(got_list) = crc_map.get(&(size, crc.clone())) {
                    Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                }
            }
            if let Some(ref crc) = missing_ref.crc {
                let alt_size = missing_ref.alt_size.unwrap_or(size);
                if alt_size != size {
                    if let Some(got_list) = crc_map.get(&(alt_size, crc.clone())) {
                        Self::extend_unique_got_candidates(
                            &mut crc_candidates,
                            got_list,
                            &mut crc_seen,
                        );
                    }
                }
            }
            if let Some(ref alt_crc) = missing_ref.alt_crc {
                let alt_size = missing_ref.alt_size.unwrap_or(size);
                if let Some(got_list) = crc_map.get(&(alt_size, alt_crc.clone())) {
                    Self::extend_unique_got_candidates(&mut crc_candidates, got_list, &mut crc_seen);
                }
            }
            if !crc_candidates.is_empty() {
                found_got_idx = Self::preferred_got_idx(&crc_candidates, &files_got, &used_got_indices);
            }

            if found_got_idx.is_none() {
                let mut sha1_candidates = Vec::new();
                let mut sha1_seen = HashSet::new();
                if let Some(ref sha1) = missing_ref.sha1 {
                    if let Some(got_list) = sha1_map.get(&(size, sha1.clone())) {
                        Self::extend_unique_got_candidates(
                            &mut sha1_candidates,
                            got_list,
                            &mut sha1_seen,
                        );
                    }
                }
                if let Some(ref sha1) = missing_ref.sha1 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if alt_size != size {
                        if let Some(got_list) = sha1_map.get(&(alt_size, sha1.clone())) {
                            Self::extend_unique_got_candidates(
                                &mut sha1_candidates,
                                got_list,
                                &mut sha1_seen,
                            );
                        }
                    }
                }
                if let Some(ref alt_sha1) = missing_ref.alt_sha1 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if let Some(got_list) = sha1_map.get(&(alt_size, alt_sha1.clone())) {
                        Self::extend_unique_got_candidates(
                            &mut sha1_candidates,
                            got_list,
                            &mut sha1_seen,
                        );
                    }
                }
                if !sha1_candidates.is_empty() {
                    found_got_idx =
                        Self::preferred_got_idx(&sha1_candidates, &files_got, &used_got_indices);
                }
            }

            if found_got_idx.is_none() {
                let mut md5_candidates = Vec::new();
                let mut md5_seen = HashSet::new();
                if let Some(ref md5) = missing_ref.md5 {
                    if let Some(got_list) = md5_map.get(&(size, md5.clone())) {
                        Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                    }
                }
                if let Some(ref md5) = missing_ref.md5 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if alt_size != size {
                        if let Some(got_list) = md5_map.get(&(alt_size, md5.clone())) {
                            Self::extend_unique_got_candidates(
                                &mut md5_candidates,
                                got_list,
                                &mut md5_seen,
                            );
                        }
                    }
                }
                if let Some(ref alt_md5) = missing_ref.alt_md5 {
                    let alt_size = missing_ref.alt_size.unwrap_or(size);
                    if let Some(got_list) = md5_map.get(&(alt_size, alt_md5.clone())) {
                        Self::extend_unique_got_candidates(&mut md5_candidates, got_list, &mut md5_seen);
                    }
                }
                if !md5_candidates.is_empty() {
                    found_got_idx =
                        Self::preferred_got_idx(&md5_candidates, &files_got, &used_got_indices);
                }
            }

            let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
            if let Some(got_idx) = found_got_idx {
                let got = &files_got[got_idx];
                let is_corrupt = got.borrow().got_status() == GotStatus::Corrupt;
                missing_ref.set_rep_status(if is_corrupt {
                    RepStatus::CorruptCanBeFixed
                } else if is_mia {
                    RepStatus::CanBeFixedMIA
                } else {
                    RepStatus::CanBeFixed
                });
                let mut got_mut = got.borrow_mut();
                let current_rep = got_mut.rep_status();
                if got_mut.got_status() != GotStatus::Corrupt
                    && (current_rep == RepStatus::UnScanned
                        || current_rep == RepStatus::InToSort
                        || current_rep == RepStatus::MoveToSort
                        || current_rep == RepStatus::Unknown
                        || current_rep == RepStatus::Deleted
                        || current_rep == RepStatus::UnNeeded)
                {
                    got_mut.set_rep_status(RepStatus::NeededForFix);
                }
                if Self::source_is_consumable(&got_mut) {
                    used_got_indices.insert(got_idx);
                }
                missing_ref.cached_stats = None;
                got_mut.cached_stats = None;
            } else {
                let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
                missing_ref.set_rep_status(if is_mia {
                    RepStatus::MissingMIA
                } else {
                    RepStatus::Missing
                });
                missing_ref.cached_stats = None;
            }
        }

        for (idx, got) in files_got.iter().enumerate() {
            let (got_status, rep_status, dat_status) = {
                let got_ref = got.borrow();
                (got_ref.got_status(), got_ref.rep_status(), got_ref.dat_status())
            };
            if got_status == GotStatus::Corrupt {
                let merged_cleanup_status =
                    if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                        Some(Self::merged_cleanup_status(idx, &files_got))
                    } else {
                        None
                    };
                let mut got_ref = got.borrow_mut();
                if rep_status == RepStatus::NeededForFix {
                } else if dat_status == DatStatus::InDatCollect {
                    got_ref.set_rep_status(RepStatus::MoveToCorrupt);
                    got_ref.cached_stats = None;
                } else if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                    got_ref.set_rep_status(merged_cleanup_status.unwrap());
                    got_ref.cached_stats = None;
                } else {
                    got_ref.set_rep_status(RepStatus::Delete);
                    got_ref.cached_stats = None;
                }
            }
        }

        for (idx, got) in files_got.iter().enumerate() {
            let (current_rep_status, dat_status) = {
                let got_ref = got.borrow();
                (got_ref.rep_status(), got_ref.dat_status())
            };
            if current_rep_status == RepStatus::NeededForFix
                || current_rep_status == RepStatus::Correct
                || current_rep_status == RepStatus::Delete
                || current_rep_status == RepStatus::MoveToCorrupt
            {
                continue;
            }
            let merged_cleanup_status =
                if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                    Some(Self::merged_cleanup_status(idx, &files_got))
                } else {
                    None
                };
            let should_delete_tosort = if dat_status == DatStatus::InToSort {
                let got_ref = got.borrow();
                Self::has_redundant_romroot_match(
                    Rc::clone(got),
                    &got_ref,
                    &all_got_files,
                    &all_got_crc_map,
                    &all_got_sha1_map,
                    &all_got_md5_map,
                ) || Self::has_redundant_physical_dat_match(Rc::clone(got), &got_ref, &all_dat_files)
                    || Self::has_pending_fix_target_match(
                        &got_ref,
                        &files_missing,
                        &missing_crc_map,
                        &missing_sha1_map,
                        &missing_md5_map,
                    )
            } else {
                false
            };
            let mut got_ref = got.borrow_mut();
            if dat_status == DatStatus::InDatCollect {
                got_ref.set_rep_status(RepStatus::Correct);
                got_ref.cached_stats = None;
            } else if matches!(dat_status, DatStatus::InDatMerged | DatStatus::InDatNoDump) {
                got_ref.set_rep_status(merged_cleanup_status.unwrap());
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::InDatMIA {
                got_ref.set_rep_status(RepStatus::CorrectMIA);
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::InToSort {
                if should_delete_tosort {
                    got_ref.set_rep_status(RepStatus::Delete);
                } else {
                    got_ref.set_rep_status(RepStatus::InToSort);
                }
                got_ref.cached_stats = None;
            } else if dat_status == DatStatus::NotInDat {
                got_ref.set_rep_status(RepStatus::MoveToSort);
                got_ref.cached_stats = None;
            }
        }
    }
}
