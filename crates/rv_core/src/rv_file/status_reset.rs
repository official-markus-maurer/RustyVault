impl RvFile {
    fn invalidate_cached_stats_with_ancestors(&mut self) {
        self.cached_stats = None;
        if self.dir_status.is_some() {
            self.dir_status = Some(ReportStatus::Unknown);
        }
        let mut current = self.parent.as_ref().and_then(|parent| parent.upgrade());
        while let Some(node_rc) = current {
            let next = {
                let Ok(mut node) = node_rc.try_borrow_mut() else {
                    break;
                };
                node.cached_stats = None;
                if node.dir_status.is_some() {
                    node.dir_status = Some(ReportStatus::Unknown);
                }
                node.parent.as_ref().and_then(|parent| parent.upgrade())
            };
            current = next;
        }
    }

    /// Partially resets the `RepStatus` and `GotStatus` to baseline values prior to a fix pass.
    pub fn rep_status_reset(&mut self) {
        // Rust port simplification of RomVaultCore/RvDB/rvFile.cs RepStatusReset
        self.search_found = false;

        // When rep_status resets, the cached_stats need to be cleared
        self.invalidate_cached_stats_with_ancestors();

        if self.file_type == FileType::File
            && self.dat_status == dat_reader::enums::DatStatus::NotInDat
            && self.got_status == dat_reader::enums::GotStatus::Got
        {
            if let Some(parent_rc) = self.parent.as_ref().and_then(|p| p.upgrade()) {
                let parent_name = parent_rc.borrow().get_full_name();
                let rule = crate::settings::find_rule(&parent_name);

                let rule_has_db_patterns = rule.ignore_files.items.iter().any(|p| {
                    crate::patterns::extract_db_pattern(p).is_some_and(|s| !s.trim().is_empty())
                });

                let patterns = if rule_has_db_patterns {
                    &rule.ignore_files.items
                } else {
                    &crate::settings::get_settings().ignore_files.items
                };

                for raw in patterns {
                    let Some(pat) = crate::patterns::extract_db_pattern(raw) else {
                        continue;
                    };
                    let pat = pat.trim();
                    if pat.is_empty() {
                        continue;
                    }

                    #[cfg(windows)]
                    {
                        let is_regex = pat.len() >= 6 && pat[..6].eq_ignore_ascii_case("regex:");
                        if is_regex {
                            if crate::patterns::matches_pattern(&self.name, pat) {
                                self.rep_status = RepStatus::Ignore;
                                return;
                            }
                        } else {
                            let name = self.name.to_ascii_lowercase();
                            let p = pat.to_ascii_lowercase();
                            if crate::patterns::matches_pattern(&name, &p) {
                                self.rep_status = RepStatus::Ignore;
                                return;
                            }
                        }
                    }
                    #[cfg(not(windows))]
                    {
                        if crate::patterns::matches_pattern(&self.name, pat) {
                            self.rep_status = RepStatus::Ignore;
                            return;
                        }
                    }
                }
            }
        }

        let new_status = match (self.file_type, self.dat_status, self.got_status) {
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatCollect,
                dat_reader::enums::GotStatus::Got,
            ) => RepStatus::Correct,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatCollect,
                dat_reader::enums::GotStatus::NotGot,
            ) => RepStatus::Missing,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatCollect,
                dat_reader::enums::GotStatus::Corrupt,
            ) => RepStatus::Corrupt,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatMerged | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::NotGot,
            ) => RepStatus::NotCollected,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatMerged | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::Got | dat_reader::enums::GotStatus::Corrupt,
            ) => RepStatus::UnNeeded,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatMIA,
                dat_reader::enums::GotStatus::NotGot,
            ) => RepStatus::MissingMIA,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InDatMIA,
                dat_reader::enums::GotStatus::Got,
            ) => RepStatus::CorrectMIA,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InToSort,
                dat_reader::enums::GotStatus::Got,
            ) => RepStatus::InToSort,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::InToSort,
                dat_reader::enums::GotStatus::NotGot,
            ) => RepStatus::Deleted,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::NotInDat,
                dat_reader::enums::GotStatus::Got,
            ) => RepStatus::Unknown,
            (
                FileType::File
                | FileType::FileZip
                | FileType::FileSevenZip
                | FileType::FileOnly
                | FileType::Zip
                | FileType::SevenZip,
                dat_reader::enums::DatStatus::NotInDat,
                dat_reader::enums::GotStatus::NotGot,
            ) => RepStatus::Deleted,
            (
                FileType::Dir,
                dat_reader::enums::DatStatus::InDatCollect
                | dat_reader::enums::DatStatus::InDatMerged
                | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::Got,
            ) => RepStatus::DirCorrect,
            (
                FileType::Dir,
                dat_reader::enums::DatStatus::InDatCollect
                | dat_reader::enums::DatStatus::InDatMerged
                | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::NotGot,
            ) => RepStatus::DirMissing,
            (
                FileType::Dir,
                dat_reader::enums::DatStatus::InDatCollect
                | dat_reader::enums::DatStatus::InDatMerged
                | dat_reader::enums::DatStatus::InDatNoDump,
                dat_reader::enums::GotStatus::Corrupt,
            ) => RepStatus::DirCorrupt,
            (
                FileType::Dir,
                dat_reader::enums::DatStatus::NotInDat,
                dat_reader::enums::GotStatus::Got,
            ) => RepStatus::DirUnknown,
            (
                FileType::Dir,
                dat_reader::enums::DatStatus::InToSort,
                dat_reader::enums::GotStatus::Got,
            ) => RepStatus::DirInToSort,
            _ => RepStatus::UnScanned,
        };
        self.rep_status = new_status;
    }
}
