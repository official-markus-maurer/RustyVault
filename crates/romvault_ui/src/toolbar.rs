use eframe::egui;
use std::rc::Rc;

use crate::RomVaultApp;
use rv_core::db::GLOBAL_DB;
use rv_core::find_fixes::FindFixes;

/// Logic for rendering the top main menu and toolbar.
/// 
/// `toolbar.rs` contains the top-level `egui::TopBottomPanel` rendering for the main
/// application, drawing buttons like `Update DATs`, `Scan ROMs`, and `Fix ROMs`.
/// 
/// Differences from C#:
/// - C# uses `ToolStrip` and `MenuStrip` objects bound to internal event handlers.
/// - Rust utilizes `egui::menu::bar` and directly triggers state transitions (or worker threads)
///   when the immediate-mode buttons report a `.clicked()` event.
pub fn draw_left_toolbar(app: &mut RomVaultApp, ctx: &egui::Context) {
    let dark_mode = ctx.style().visuals.dark_mode;
    let panel_fill = if dark_mode {
        egui::Color32::from_rgb(20, 20, 22)
    } else {
        egui::Color32::from_rgb(246, 246, 248)
    };
    let panel_stroke = if dark_mode {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 45))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 226))
    };
    egui::SidePanel::left("left_toolbar_panel")
        .exact_width(80.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(panel_fill)
                .inner_margin(0.0)
                .stroke(panel_stroke),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            {
                let visuals = &mut ui.style_mut().visuals;
                visuals.widgets.noninteractive.rounding = egui::Rounding::ZERO;
                visuals.widgets.inactive.rounding = egui::Rounding::ZERO;
                visuals.widgets.hovered.rounding = egui::Rounding::ZERO;
                visuals.widgets.active.rounding = egui::Rounding::ZERO;

                visuals.widgets.noninteractive.expansion = 0.0;
                visuals.widgets.inactive.expansion = 0.0;
                visuals.widgets.hovered.expansion = 0.0;
                visuals.widgets.active.expansion = 0.0;

                visuals.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.active.bg_fill = egui::Color32::TRANSPARENT;
                visuals.widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;

                visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
            }

            egui::TopBottomPanel::bottom("left_toolbar_bottom")
                .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(0.0, 5.0)))
                .show_inside(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("Tree\nPre-Sets").size(10.0));
                        ui.add_space(2.0);
                        let btn_size = egui::vec2(36.0, 36.0);

                        ui.horizontal(|ui| {
                            ui.add_space(2.0);
                            let btn1 = egui::ImageButton::new(
                                egui::Image::new(include_asset!("default1.png"))
                                    .fit_to_original_size(1.0)
                                    .texture_options(egui::TextureOptions::NEAREST),
                            )
                            .frame(false);
                            let resp1 = ui
                                .add_sized(btn_size, btn1)
                                .on_hover_text("Right Click: Save Tree Settings\nLeft Click: Load Tree Settings");
                            if resp1.clicked() {
                                app.load_tree_preset(1);
                                app.show_complete = true;
                                app.show_partial = true;
                                app.show_empty = true;
                                app.show_fixes = true;
                                app.show_mia = true;
                                app.show_merged = true;
                                app.persist_filter_settings();
                            } else if resp1.secondary_clicked() {
                                app.save_tree_preset(1);
                            }

                            let btn2 = egui::ImageButton::new(
                                egui::Image::new(include_asset!("default2.png"))
                                    .fit_to_original_size(1.0)
                                    .texture_options(egui::TextureOptions::NEAREST),
                            )
                            .frame(false);
                            let resp2 = ui
                                .add_sized(btn_size, btn2)
                                .on_hover_text("Right Click: Save Tree Settings\nLeft Click: Load Tree Settings");
                            if resp2.clicked() {
                                app.load_tree_preset(2);
                                app.show_complete = true;
                                app.show_partial = false;
                                app.show_empty = false;
                                app.show_fixes = false;
                                app.show_mia = false;
                                app.show_merged = true;
                                app.persist_filter_settings();
                            } else if resp2.secondary_clicked() {
                                app.save_tree_preset(2);
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.add_space(2.0);
                            let btn3 = egui::ImageButton::new(
                                egui::Image::new(include_asset!("default3.png"))
                                    .fit_to_original_size(1.0)
                                    .texture_options(egui::TextureOptions::NEAREST),
                            )
                            .frame(false);
                            let resp3 = ui
                                .add_sized(btn_size, btn3)
                                .on_hover_text("Right Click: Save Tree Settings\nLeft Click: Load Tree Settings");
                            if resp3.clicked() {
                                app.load_tree_preset(3);
                                app.show_complete = false;
                                app.show_partial = true;
                                app.show_empty = false;
                                app.show_fixes = true;
                                app.show_mia = true;
                                app.show_merged = false;
                                app.persist_filter_settings();
                            } else if resp3.secondary_clicked() {
                                app.save_tree_preset(3);
                            }

                            let btn4 = egui::ImageButton::new(
                                egui::Image::new(include_asset!("default4.png"))
                                    .fit_to_original_size(1.0)
                                    .texture_options(egui::TextureOptions::NEAREST),
                            )
                            .frame(false);
                            let resp4 = ui
                                .add_sized(btn_size, btn4)
                                .on_hover_text("Right Click: Save Tree Settings\nLeft Click: Load Tree Settings");
                            if resp4.clicked() {
                                app.load_tree_preset(4);
                                app.show_complete = false;
                                app.show_partial = false;
                                app.show_empty = false;
                                app.show_fixes = true;
                                app.show_mia = false;
                                app.show_merged = false;
                                app.persist_filter_settings();
                            } else if resp4.secondary_clicked() {
                                app.save_tree_preset(4);
                            }
                        });
                    });
                });

            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical_centered_justified(|ui| {
                            let is_idle = !app.sam_running;
                            ui.add_enabled_ui(is_idle, |ui| {
                                let btn_update_img = if is_idle {
                                    include_toolbar_image!("btnUpdateDats_Enabled.png")
                                } else {
                                    include_toolbar_image!("btnUpdateDats_Disabled.png")
                                };
                                let update_resp = ui
                                    .add_sized(
                                        [ui.available_width(), 70.0],
                                        egui::ImageButton::new(
                                            egui::Image::new(btn_update_img)
                                                .fit_to_original_size(1.0)
                                                .texture_options(egui::TextureOptions::NEAREST),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text(
                                        "Left Click: Dat Update\nShift Left Click: Full Dat Rescan\n\nRight Click: Open DatVault",
                                    );
                                if update_resp.clicked() {
                                    app.update_dats(ui.input(|i| i.modifiers.shift));
                                } else if update_resp.secondary_clicked() {
                                    let dat_root = rv_core::settings::get_settings().dat_root;
                                    let dat_root_path = if dat_root.is_empty() { "DatRoot" } else { &dat_root };
                                    app.task_logs.push("Opening DatVault".to_string());
                                    let _ = std::process::Command::new("cmd")
                                        .args(["/C", "start", "", dat_root_path])
                                        .spawn();
                                }

                                let btn_scan_img = if is_idle {
                                    include_toolbar_image!("btnScanRoms_Enabled.png")
                                } else {
                                    include_toolbar_image!("btnScanRoms_Disabled.png")
                                };
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 70.0],
                                            egui::ImageButton::new(
                                                egui::Image::new(btn_scan_img)
                                                    .fit_to_original_size(1.0)
                                                    .texture_options(egui::TextureOptions::NEAREST),
                                            )
                                            .frame(false),
                                    )
                                    .clicked()
                                {
                                    app.launch_scan_roms_task(
                                        "Scan ROMs",
                                        "Scanning selected ROM roots...",
                                        rv_core::settings::EScanLevel::Level2,
                                    );
                                }

                                let btn_find_img = if is_idle {
                                    include_toolbar_image!("btnFindFixes_Enabled.png")
                                } else {
                                    include_toolbar_image!("btnFindFixes_Disabled.png")
                                };
                                let find_resp = ui
                                    .add_sized(
                                        [ui.available_width(), 70.0],
                                        egui::ImageButton::new(
                                            egui::Image::new(btn_find_img)
                                                .fit_to_original_size(1.0)
                                                .texture_options(egui::TextureOptions::NEAREST),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Left Click: Find Fixes\nCtrl+Shift+Left Click: Find Fixes (Advanced Logging)");
                                if find_resp.clicked() {
                                    let is_advanced = ui.input(|i| i.modifiers.ctrl && i.modifiers.shift);
                                    app.launch_task("Find Fixes", move |tx| {
                                        if is_advanced {
                                            let _ = tx.send("Running FindFixes (Advanced Logging Enabled)...".to_string());
                                        } else {
                                            let _ = tx.send("Running FindFixes...".to_string());
                                        }
                                        GLOBAL_DB.with(|db_ref| {
                                            if let Some(db) = db_ref.borrow().as_ref() {
                                                FindFixes::scan_files(Rc::clone(&db.dir_root));
                                                rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
                                                db.write_cache();
                                            }
                                        });
                                    });
                                }

                                let btn_fix_img = if is_idle {
                                    include_toolbar_image!("btnFixFiles_Enabled.png")
                                } else {
                                    include_toolbar_image!("btnFixFiles_Disabled.png")
                                };
                                let fix_resp = ui
                                    .add_sized(
                                        [ui.available_width(), 70.0],
                                        egui::ImageButton::new(
                                            egui::Image::new(btn_fix_img)
                                                .fit_to_original_size(1.0)
                                                .texture_options(egui::TextureOptions::NEAREST),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Left Click: Fix Files\nRight Click: Scan / Find Fix / Fix");
                                if fix_resp.clicked() {
                                    app.launch_fix_roms_task();
                                } else if fix_resp.secondary_clicked() {
                                    app.launch_scan_find_fix_fix_task();
                                }

                                let btn_report_img = if is_idle {
                                    include_toolbar_image!("btnReport_Enabled.png")
                                } else {
                                    include_toolbar_image!("btnReport_Disabled.png")
                                };
                                let report_resp = ui
                                    .add_sized(
                                        [ui.available_width(), 70.0],
                                        egui::ImageButton::new(
                                            egui::Image::new(btn_report_img)
                                                .fit_to_original_size(1.0)
                                                .texture_options(egui::TextureOptions::NEAREST),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text(
                                        "Left Click: Generate FixDATs (Missing/MIA only)\nRight Click: Generate FixDATs (Missing/MIA + Fixable)",
                                    );
                                if report_resp.clicked() {
                                    app.prompt_fixdat_report(true);
                                } else if report_resp.secondary_clicked() {
                                    app.prompt_fixdat_report(false);
                                }
                            });
                        });
                    });
                });
        });
}

