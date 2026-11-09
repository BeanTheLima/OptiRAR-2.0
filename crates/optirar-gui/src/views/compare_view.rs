use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Vec2};
use optirar_core::format_bytes;
use optirar_core::t;
use optirar_engine::ArchiveDiffResult;
use std::path::PathBuf;

#[derive(Default)]
pub struct CompareViewState {
    pub archive_a: Option<PathBuf>,
    pub archive_b: Option<PathBuf>,
    pub show_identical: bool,
}

pub struct CompareView;

impl CompareView {
    pub fn show(
        ui: &mut egui::Ui,
        state: &mut CompareViewState,
        diff: Option<&ArchiveDiffResult>,
        is_comparing: bool,
        on_start_compare: impl FnOnce(PathBuf, PathBuf),
    ) {
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("⚖ {}", t("tools.archive_diff"))).size(18.0).strong());
            });
            ui.label(RichText::new(t("tools.archive_diff_desc")).size(12.0).color(ui.visuals().weak_text_color()));
            ui.add_space(10.0);

            // Select Archive A
            ui.horizontal(|ui| {
                ui.add_sized(Vec2::new(90.0, 24.0), egui::Label::new(RichText::new(t("dialogs.compare.archive_a")).strong()));
                if ui.button(t("dialogs.extract.browse")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        state.archive_a = Some(path);
                    }
                }
                if let Some(p) = &state.archive_a {
                    ui.label(RichText::new(p.file_name().and_then(|n| n.to_str()).unwrap_or("A")).color(crate::theme::accent_text(ui)));
                } else {
                    ui.label(RichText::new(t("common.none_selected")).color(ui.visuals().weak_text_color()));
                }
            });

            // Select Archive B
            ui.horizontal(|ui| {
                ui.add_sized(Vec2::new(90.0, 24.0), egui::Label::new(RichText::new(t("dialogs.compare.archive_b")).strong()));
                if ui.button(t("dialogs.extract.browse")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        state.archive_b = Some(path);
                    }
                }
                if let Some(p) = &state.archive_b {
                    ui.label(RichText::new(p.file_name().and_then(|n| n.to_str()).unwrap_or("B")).color(crate::theme::accent_text(ui)));
                } else {
                    ui.label(RichText::new(t("common.none_selected")).color(ui.visuals().weak_text_color()));
                }
            });

            ui.add_space(8.0);

            let can_compare = state.archive_a.is_some() && state.archive_b.is_some() && !is_comparing;
            if ui
                .add_enabled(
                    can_compare,
                    egui::Button::new(RichText::new(format!("⚡ {}", t("tools.start_diff"))).strong().color(Color32::WHITE))
                        .fill(Color32::from_rgb(0, 120, 212)),
                )
                .clicked()
            {
                if let (Some(a), Some(b)) = (state.archive_a.clone(), state.archive_b.clone()) {
                    on_start_compare(a, b);
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            if is_comparing {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t("status.loading"));
                });
            } else if let Some(d) = diff {
                // Summary stat cards
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(12.0, 0.0);

                    // Identical
                    egui::Frame::NONE
                        .fill(ui.visuals().faint_bg_color)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("{}: {}", t("tools.identical"), d.identical.len())).strong().color(Color32::from_rgb(46, 204, 113)));
                        });

                    // Modified
                    egui::Frame::NONE
                        .fill(ui.visuals().faint_bg_color)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("{}: {}", t("tools.modified"), d.modified.len())).strong().color(Color32::from_rgb(241, 196, 15)));
                        });

                    // Only in A
                    egui::Frame::NONE
                        .fill(ui.visuals().faint_bg_color)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("{}: {}", t("tools.only_a"), d.only_in_a.len())).strong().color(Color32::from_rgb(231, 76, 60)));
                        });

                    // Only in B
                    egui::Frame::NONE
                        .fill(ui.visuals().faint_bg_color)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("{}: {}", t("tools.only_b"), d.only_in_b.len())).strong().color(Color32::from_rgb(52, 152, 219)));
                        });
                });

                ui.add_space(10.0);
                ui.checkbox(&mut state.show_identical, t("tools.show_identical"));
                ui.add_space(8.0);

                // Diff listings
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (a, b) in &d.modified {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t("tools.modified")).strong().size(11.0).color(Color32::from_rgb(241, 196, 15)));
                            ui.label(RichText::new(&a.path).strong().size(12.5));
                            ui.label(format!("(A: {} vs B: {})", format_bytes(a.uncompressed_size), format_bytes(b.uncompressed_size)));
                        });
                        ui.add_space(2.0);
                    }

                    for a in &d.only_in_a {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t("tools.only_a")).strong().size(11.0).color(Color32::from_rgb(231, 76, 60)));
                            ui.label(RichText::new(&a.path).size(12.5));
                            ui.label(format!("({})", format_bytes(a.uncompressed_size)));
                        });
                        ui.add_space(2.0);
                    }

                    for b in &d.only_in_b {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t("tools.only_b")).strong().size(11.0).color(Color32::from_rgb(52, 152, 219)));
                            ui.label(RichText::new(&b.path).size(12.5));
                            ui.label(format!("({})", format_bytes(b.uncompressed_size)));
                        });
                        ui.add_space(2.0);
                    }

                    if state.show_identical {
                        for item in &d.identical {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(t("tools.identical")).size(11.0).color(ui.visuals().weak_text_color()));
                                ui.label(RichText::new(&item.path).size(12.0).color(ui.visuals().weak_text_color()));
                            });
                            ui.add_space(2.0);
                        }
                    }
                });
            }
        });
    }
}
