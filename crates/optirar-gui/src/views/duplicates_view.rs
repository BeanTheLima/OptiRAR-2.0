use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Vec2};
use optirar_core::format_bytes;
use optirar_core::t;
use optirar_engine::DuplicateFinder;
use optirar_index::catalog::ArchiveCatalog;

pub struct DuplicatesView;

impl DuplicatesView {
    pub fn show(
        ui: &mut egui::Ui,
        catalog: Option<&ArchiveCatalog>,
    ) {
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("🔍 {}", t("tools.dup_finder"))).size(18.0).strong());
            });
            ui.label(RichText::new(t("tools.dup_finder_desc")).size(12.0).color(ui.visuals().weak_text_color()));
            ui.add_space(10.0);

            if let Some(cat) = catalog {
                let report = DuplicateFinder::find_duplicates(&cat.entries);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(12.0, 0.0);

                    // Duplicate count card
                    egui::Frame::NONE
                        .fill(ui.visuals().faint_bg_color)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(14, 8))
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("{}: {}", t("tools.dup_files"), report.total_duplicate_files)).strong().color(Color32::from_rgb(241, 196, 15)));
                        });

                    // Wasted bytes card
                    egui::Frame::NONE
                        .fill(ui.visuals().faint_bg_color)
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(14, 8))
                        .show(ui, |ui| {
                            ui.label(RichText::new(format!("{}: {}", t("tools.wasted"), format_bytes(report.total_wasted_bytes))).strong().color(Color32::from_rgb(231, 76, 60)));
                        });
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                if report.groups.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        ui.label(RichText::new(format!("✓ {}", t("dialogs.duplicates.no_duplicates"))).size(14.0).color(Color32::from_rgb(46, 204, 113)));
                    });
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (idx, group) in report.groups.iter().enumerate() {
                            egui::Frame::NONE
                                .fill(ui.visuals().faint_bg_color)
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("{} #{}", t("tools.group"), idx + 1)).strong().color(crate::theme::accent_text(ui)));
                                        ui.separator();
                                        ui.label(format!("{}: {}", t("table.size"), format_bytes(group.uncompressed_size)));
                                        ui.separator();
                                        ui.label(format!("CRC32: {:08X?}", group.crc32.unwrap_or(0)));
                                        ui.separator();
                                        ui.label(RichText::new(format!("{}: {}", t("tools.wasted"), format_bytes(group.wasted_bytes))).color(Color32::from_rgb(231, 76, 60)));
                                    });

                                    ui.add_space(4.0);
                                    for entry in &group.entries {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("•").color(ui.visuals().weak_text_color()));
                                            ui.label(RichText::new(&entry.path).size(12.0));
                                        });
                                    }
                                });
                            ui.add_space(6.0);
                        }
                    });
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(RichText::new(t("table.no_archive_title")).size(14.0).color(ui.visuals().weak_text_color()));
                });
            }
        });
    }
}
