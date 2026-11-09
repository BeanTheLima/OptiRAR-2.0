use eframe::egui::{self, RichText};
use optirar_core::i18n::t_format;
use optirar_core::t;
use optirar_core::types::format_bytes;
use optirar_index::catalog::ArchiveCatalog;
use std::collections::HashSet;

use crate::theme;

pub struct StatusBarView;

impl StatusBarView {
    pub fn show(
        ui: &mut egui::Ui,
        catalog: Option<&ArchiveCatalog>,
        selected_indices: &HashSet<usize>,
        status_message: &str,
        is_error: bool,
    ) {
        ui.horizontal_centered(|ui| {
            if let Some(cat) = catalog {
                ui.label(t_format(
                    "status.items_count",
                    &[("files", &cat.file_count.to_string()), ("dirs", &cat.dir_count.to_string())],
                ));
                ui.separator();
                ui.label(t_format("status.total_size", &[("size", &format_bytes(cat.total_uncompressed_bytes))]));

                let (count, bytes) = selected_indices
                    .iter()
                    .filter_map(|&i| cat.entries.get(i))
                    .filter(|e| !e.is_directory)
                    .fold((0usize, 0u64), |(c, b), e| (c + 1, b + e.uncompressed_size));
                if count > 0 {
                    ui.separator();
                    ui.label(
                        RichText::new(t_format(
                            "status.selected_count",
                            &[("count", &count.to_string()), ("bytes", &format_bytes(bytes))],
                        ))
                        .color(theme::accent_text(ui)),
                    );
                }
                ui.separator();
                ui.label(format!("{:?}", cat.format));
                if cat.is_solid {
                    ui.label(t("status.solid_yes"));
                }
            } else {
                ui.label(t("status.ready"));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !status_message.is_empty() {
                    let color = if is_error { theme::ERROR } else { theme::SUCCESS };
                    ui.add(egui::Label::new(RichText::new(status_message).color(color)).truncate())
                        .on_hover_text(status_message);
                }
            });
        });
    }
}
