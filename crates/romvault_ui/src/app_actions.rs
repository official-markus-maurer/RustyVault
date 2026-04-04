use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Sender};

use dat_reader::enums::FileType;
use eframe::egui;
use rv_core::db::GLOBAL_DB;
use rv_core::file_scanning::FileScanning;
use rv_core::fix::Fix;
use rv_core::read_dat::DatUpdate;
use rv_core::rv_file::{RvFile, TreeSelect};
use rv_core::scanner::Scanner;

use crate::RomVaultApp;

impl RomVaultApp {
    pub(crate) fn persist_filter_settings(&mut self) {
        let mut settings = rv_core::settings::get_settings();
        settings.chk_box_show_complete = self.show_complete;
        settings.chk_box_show_partial = self.show_partial;
        settings.chk_box_show_empty = self.show_empty;
        settings.chk_box_show_fixes = self.show_fixes;
        settings.chk_box_show_mia = self.show_mia;
        settings.chk_box_show_merged = self.show_merged;

        rv_core::settings::update_settings(settings.clone());
        let _ = rv_core::settings::write_settings_to_file(&settings);
        self.global_settings = settings;
    }

    pub(crate) fn prompt_add_tosort(&mut self) {
        if self.sam_running {
            return;
        }

        let Some(folder) = rfd::FileDialog::new()
            .set_title("Select new ToSort Folder")
            .pick_folder()
        else {
            return;
        };

        let path = folder.to_string_lossy().to_string();
        self.task_logs
            .push(format!("Add ToSort folder requested: {}", path));
        rv_core::db::GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                let ts = std::rc::Rc::new(std::cell::RefCell::new(rv_core::rv_file::RvFile::new(
                    dat_reader::enums::FileType::Dir,
                )));
                {
                    let mut t = ts.borrow_mut();
                    t.name = path;
                    t.set_dat_status(dat_reader::enums::DatStatus::InToSort);
                }
                db.dir_root.borrow_mut().child_add(ts);
                rv_core::repair_status::RepairStatus::report_status_reset(std::rc::Rc::clone(
                    &db.dir_root,
                ));
            }
        });
        self.db_cache_dirty = true;
    }

    pub(crate) fn prompt_fix_report(&mut self) {
        if self.sam_running {
            return;
        }

        let Some(path) = rfd::FileDialog::new()
            .set_title("Generate Fix Report")
            .set_file_name("RVFixReport.txt")
            .add_filter("Rom Vault Fixing Report", &["txt"])
            .save_file()
        else {
            return;
        };

        let path_str = path.to_string_lossy().to_string();
        self.launch_task("Generate Reports (Fix)", move |tx| {
            let _ = tx.send(format!("Generating Fix Report to {path_str}..."));
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    if let Err(e) =
                        crate::reports::write_fix_report(&path_str, Rc::clone(&db.dir_root))
                    {
                        let _ = tx.send(format!("Failed to write Fix Report: {e}"));
                    }
                }
            });
        });
    }

    pub(crate) fn prompt_full_report(&mut self) {
        if self.sam_running {
            return;
        }

        let Some(path) = rfd::FileDialog::new()
            .set_title("Generate Full Report")
            .set_file_name("RVFullReport.txt")
            .add_filter("Rom Vault Report", &["txt"])
            .save_file()
        else {
            return;
        };

        let path_str = path.to_string_lossy().to_string();
        self.launch_task("Generate Reports (Full)", move |tx| {
            let _ = tx.send(format!("Generating Full Report to {path_str}..."));
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    if let Err(e) =
                        crate::reports::write_full_report(&path_str, Rc::clone(&db.dir_root))
                    {
                        let _ = tx.send(format!("Failed to write Full Report: {e}"));
                    }
                }
            });
        });
    }

    pub(crate) fn prompt_fixdat_report(&mut self, red_only: bool) {
        if self.sam_running {
            return;
        }

        let settings = rv_core::settings::get_settings();
        let mut dlg = rfd::FileDialog::new();
        dlg = if red_only {
            dlg.set_title("Select FixDAT output folder (Missing/MIA only)")
        } else {
            dlg.set_title("Select FixDAT output folder (Missing/MIA + Fixable)")
        };
        if let Some(default_dir) = settings
            .fix_dat_out_path
            .as_ref()
            .filter(|p| !p.trim().is_empty())
        {
            dlg = dlg.set_directory(default_dir);
        }

        let Some(folder) = dlg.pick_folder() else {
            return;
        };

        let out_dir = folder.to_string_lossy().to_string();
        let mut new_settings = settings.clone();
        if new_settings.fix_dat_out_path.as_deref() != Some(&out_dir) {
            new_settings.fix_dat_out_path = Some(out_dir.clone());
            rv_core::settings::update_settings(new_settings.clone());
            let _ = rv_core::settings::write_settings_to_file(&new_settings);
            self.global_settings = new_settings;
        }

        self.launch_task(
            if red_only {
                "Generate FixDATs (Missing)"
            } else {
                "Generate FixDATs (All)"
            },
            move |tx| {
                let _ = tx.send(format!("Generating FixDATs to {out_dir}..."));
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        rv_core::fix_dat_report::FixDatReport::recursive_dat_tree(
                            &out_dir,
                            Rc::clone(&db.dir_root),
                            red_only,
                        );
                    }
                });
            },
        );
    }

    pub(crate) fn prompt_fixdat_report_for_node(
        &mut self,
        red_only: bool,
        base_dir: Rc<RefCell<RvFile>>,
    ) {
        if self.sam_running {
            return;
        }

        let settings = rv_core::settings::get_settings();
        let mut dlg = rfd::FileDialog::new();
        dlg = if red_only {
            dlg.set_title("Select FixDAT output folder (Missing/MIA only)")
        } else {
            dlg.set_title("Select FixDAT output folder (Missing/MIA + Fixable)")
        };
        if let Some(default_dir) = settings
            .fix_dat_out_path
            .as_ref()
            .filter(|p| !p.trim().is_empty())
        {
            dlg = dlg.set_directory(default_dir);
        }

        let Some(folder) = dlg.pick_folder() else {
            return;
        };

        let out_dir = folder.to_string_lossy().to_string();
        let mut new_settings = settings.clone();
        if new_settings.fix_dat_out_path.as_deref() != Some(&out_dir) {
            new_settings.fix_dat_out_path = Some(out_dir.clone());
            rv_core::settings::update_settings(new_settings.clone());
            let _ = rv_core::settings::write_settings_to_file(&new_settings);
            self.global_settings = new_settings;
        }

        self.launch_task(
            if red_only {
                "Generate FixDATs (Missing)"
            } else {
                "Generate FixDATs (All)"
            },
            move |tx| {
                let _ = tx.send(format!("Generating FixDATs to {out_dir}..."));
                rv_core::fix_dat_report::FixDatReport::recursive_dat_tree(
                    &out_dir, base_dir, red_only,
                );
            },
        );
    }

    pub(crate) fn prompt_make_dat(&mut self, node_rc: Rc<RefCell<RvFile>>) {
        if self.sam_running {
            return;
        }

        let default_name = {
            let n = node_rc.borrow();
            if n.name.is_empty() {
                "export.dat".to_string()
            } else if n.name.to_ascii_lowercase().ends_with(".dat") {
                n.name.clone()
            } else {
                format!("{}.dat", n.name)
            }
        };

        let Some(path) = rfd::FileDialog::new()
            .set_title("Save a Dat File")
            .set_file_name(&default_name)
            .add_filter("DAT file", &["dat"])
            .save_file()
        else {
            return;
        };

        let path_str = path.to_string_lossy().to_string();
        self.launch_task("Make DAT", move |tx| {
            let _ = tx.send(format!("Writing DAT to {path_str}..."));
            let converter = rv_core::external_dat_converter_to::ExternalDatConverterTo {
                filter_merged: true,
                ..rv_core::external_dat_converter_to::ExternalDatConverterTo::new()
            };
            let Some(dh) = converter.convert_to_external_dat(Rc::clone(&node_rc)) else {
                let _ = tx.send("Make DAT failed: not a directory node".to_string());
                return;
            };

            match std::fs::File::create(&path_str) {
                Ok(mut f) => {
                    if let Err(e) = dat_reader::xml_writer::DatXmlWriter::write_dat(&mut f, &dh) {
                        let _ = tx.send(format!("Make DAT failed: {e}"));
                    }
                }
                Err(e) => {
                    let _ = tx.send(format!("Make DAT failed: {e}"));
                }
            }
        });
    }

    pub(crate) fn update_dats(&mut self, check_all: bool) {
        if self.sam_running {
            return;
        }

        let dat_root = rv_core::settings::get_settings().dat_root;
        let dat_root_path = if dat_root.is_empty() {
            "DatRoot".to_string()
        } else {
            dat_root
        };

        self.launch_task(
            if check_all {
                "Update All DATs"
            } else {
                "Update DATs"
            },
            move |tx| {
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        let _ = tx.send(format!("Scanning {}...", dat_root_path));
                        if check_all {
                            let _ = tx.send("Full DAT rescan...".to_string());
                            DatUpdate::check_all_dats(Rc::clone(&db.dir_root), &dat_root_path);
                        }
                        DatUpdate::update_dat(Rc::clone(&db.dir_root), &dat_root_path);
                        rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(
                            &db.dir_root,
                        ));
                        db.dir_root.borrow_mut().cached_stats = None;
                    }
                });
            },
        );
    }

    pub(crate) fn flush_db_cache_if_needed(&mut self) {
        if self.sam_running {
            return;
        }
        if !self.db_cache_dirty {
            return;
        }

        let now = std::time::Instant::now();
        let should_write = self
            .db_cache_last_write
            .map(|last| now.duration_since(last) >= std::time::Duration::from_millis(500))
            .unwrap_or(true);
        if !should_write {
            return;
        }

        GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                db.write_cache();
            }
        });
        self.db_cache_dirty = false;
        self.db_cache_last_write = Some(now);
    }

    pub(crate) fn garbage_collect(&mut self, ctx: &egui::Context) {
        self.loaded_artwork = None;
        self.loaded_logo = None;
        self.loaded_title = None;
        self.loaded_screen = None;
        self.loaded_info = None;
        self.loaded_info_type.clear();

        self.show_rom_info = false;
        self.selected_rom_for_info = None;
        self.rom_info_lines.clear();

        ctx.memory_mut(|mem| *mem = Default::default());
    }

    pub(crate) fn open_dir_mappings(&mut self) {
        self.global_settings = rv_core::settings::get_settings();
        self.working_dir_mappings = self.global_settings.dir_mappings.items.clone();
        self.selected_dir_mapping_idx = None;
        self.show_dir_mappings = true;
    }

    pub(crate) fn launch_task<F>(&mut self, task_name: &str, f: F)
    where
        F: FnOnce(Sender<String>) + 'static,
    {
        let (tx, rx) = channel();
        let selection_chain = self
            .selected_node
            .as_ref()
            .map(crate::full_name_chain_from_node)
            .unwrap_or_default();

        self.task_logs.push(format!("Starting {}...", task_name));

        f(tx.clone());

        while let Ok(msg) = rx.try_recv() {
            self.task_logs.push(msg);
        }

        if !selection_chain.is_empty() {
            let mut found_any = false;
            GLOBAL_DB.with(|db_ref| {
                let binding = db_ref.borrow();
                let Some(db) = binding.as_ref() else { return };
                for key in &selection_chain {
                    if let Some(found) = crate::find_node_by_full_name_key(&db.dir_root, key) {
                        found_any = true;
                        self.select_node(found);
                        break;
                    }
                }
            });
            if !found_any {
                self.selected_node = None;
            }
        }

        self.task_logs.push("Saving DB Cache...".to_string());
        GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                db.write_cache();
            }
        });

        self.task_logs.push("Task completed.".to_string());
    }

    fn scan_selected_roots(tx: &Sender<String>, scan_level: rv_core::settings::EScanLevel) {
        GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                let settings = rv_core::settings::get_settings();
                let mut cache_timer = if settings.cache_save_timer_enabled {
                    Some(std::time::Instant::now())
                } else {
                    None
                };

                let root_children = db.dir_root.borrow().children.clone();

                for child in root_children {
                    let (name, is_selected) = {
                        let node = child.borrow();
                        (node.name.clone(), Self::branch_has_selected_nodes(&node))
                    };

                    if !is_selected {
                        continue;
                    }

                    let _ = tx.send(format!("Scanning {}...", name));
                    let physical_path =
                        rv_core::settings::find_dir_mapping(&name).unwrap_or(name.clone());
                    let rule = rv_core::settings::find_rule(&name);
                    let files = Scanner::scan_directory_with_level_and_ignore(
                        &physical_path,
                        scan_level,
                        &rule.ignore_files.items,
                    );
                    let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                    root_scan.name = name.clone();
                    root_scan.children = files;
                    let _ = tx.send(format!("Integrating {} files into DB...", name));
                    FileScanning::scan_dir_with_level(
                        Rc::clone(&child),
                        &mut root_scan,
                        scan_level,
                    );

                    if let Some(last) = cache_timer {
                        if last.elapsed().as_secs_f64() / 60.0
                            > settings.cache_save_time_period as f64
                        {
                            let _ = tx.send("Saving Cache".to_string());
                            db.write_cache();
                            let _ = tx.send("Saving Cache Complete".to_string());
                            cache_timer = Some(std::time::Instant::now());
                        } else {
                            cache_timer = Some(last);
                        }
                    }
                }

                rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
            }
        });
    }

    pub(crate) fn branch_has_selected_nodes(node: &RvFile) -> bool {
        if matches!(node.tree_checked, TreeSelect::Selected | TreeSelect::Locked) {
            return true;
        }

        for child in &node.children {
            if Self::branch_has_selected_nodes(&child.borrow()) {
                return true;
            }
        }

        false
    }

    pub(crate) fn launch_scan_roms_task(
        &mut self,
        task_name: &'static str,
        status_message: &'static str,
        scan_level: rv_core::settings::EScanLevel,
    ) {
        self.launch_task(task_name, move |tx| {
            let _ = tx.send(status_message.to_string());
            Self::scan_selected_roots(&tx, scan_level);
        });
    }

    pub(crate) fn launch_fix_roms_task(&mut self) {
        self.launch_task("Fix ROMs", |tx| {
            let _ = tx.send("Rescanning to refresh fix plan...".to_string());
            Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);

            for pass in 1..=4 {
                let _ = tx.send(format!("Finding Fixes (pass {pass}/4)..."));
                let pending = GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        crate::recompute_fix_plan(Rc::clone(&db.dir_root));
                        crate::current_fixable_count(Rc::clone(&db.dir_root))
                    } else {
                        0
                    }
                });

                if pending == 0 {
                    break;
                }

                let _ = tx.send(format!("Performing physical fixes (pass {pass}/4)..."));
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        rv_core::task_reporter::set_task_reporter(tx.clone());
                        Fix::perform_fixes(Rc::clone(&db.dir_root));
                        rv_core::task_reporter::clear_task_reporter();
                    }
                });

                let _ = tx.send(format!(
                    "Rescanning to sync DB with disk (pass {pass}/4)..."
                ));
                Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);
            }

            let _ = tx.send("Refreshing final repair state...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    crate::recompute_fix_plan(Rc::clone(&db.dir_root));
                    rv_core::task_reporter::set_task_reporter(tx.clone());
                    rv_core::report_found_mia(Rc::clone(&db.dir_root));
                    rv_core::task_reporter::clear_task_reporter();
                }
            });
        });
    }

    pub(crate) fn launch_scan_find_fix_fix_task(&mut self) {
        self.launch_task("Scan / Find Fix / Fix", |tx| {
            let _ = tx.send("Full automated fix routine started...".to_string());

            let _ = tx.send("Scanning...".to_string());
            Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);

            for pass in 1..=4 {
                let _ = tx.send(format!("Finding Fixes (pass {pass}/4)..."));
                let pending = GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        crate::recompute_fix_plan(Rc::clone(&db.dir_root));
                        crate::current_fixable_count(Rc::clone(&db.dir_root))
                    } else {
                        0
                    }
                });

                if pending == 0 {
                    break;
                }

                let _ = tx.send(format!("Fixing (pass {pass}/4)..."));
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        rv_core::task_reporter::set_task_reporter(tx.clone());
                        Fix::perform_fixes(Rc::clone(&db.dir_root));
                        rv_core::task_reporter::clear_task_reporter();
                    }
                });

                let _ = tx.send(format!(
                    "Rescanning to sync DB with disk (pass {pass}/4)..."
                ));
                Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);
            }

            let _ = tx.send("Refreshing final repair state...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    crate::recompute_fix_plan(Rc::clone(&db.dir_root));
                    rv_core::task_reporter::set_task_reporter(tx.clone());
                    rv_core::report_found_mia(Rc::clone(&db.dir_root));
                    rv_core::task_reporter::clear_task_reporter();
                }
            });
        });
    }

    pub(crate) fn load_tree_preset(&mut self, preset_index: i32) {
        if self.sam_running {
            return;
        }

        let filename = format!("treeDefault{}.xml", preset_index);
        let Some(entries) = crate::tree_presets::read_preset_file(&filename) else {
            self.task_logs
                .push(format!("Preset {} not found", preset_index));
            return;
        };

        GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                crate::tree_presets::apply_tree_state(Rc::clone(&db.dir_root), &entries);
            }
        });
        self.db_cache_dirty = true;
        self.task_logs
            .push(format!("Loaded Tree Preset {}", preset_index));
    }

    pub(crate) fn save_tree_preset(&mut self, preset_index: i32) {
        if self.sam_running {
            return;
        }

        let mut entries = Vec::new();
        GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                entries = crate::tree_presets::collect_tree_state(Rc::clone(&db.dir_root));
            }
        });

        let filename = format!("treeDefault{}.xml", preset_index);
        let _ = crate::tree_presets::write_preset_file(&filename, &entries);
        self.task_logs
            .push(format!("Saved Tree Preset {}", preset_index));
    }
}
