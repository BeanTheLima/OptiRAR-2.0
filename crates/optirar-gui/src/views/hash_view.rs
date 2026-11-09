use eframe::egui::{self, Color32, RichText, Vec2};
use optirar_core::format_bytes;
use optirar_core::t;
use optirar_integrity::FileHashes;
use std::path::PathBuf;

#[derive(Default)]
pub struct HashViewState {
    pub selected_file: Option<PathBuf>,
    pub compare_input: String,
    pub copied_indicator: Option<String>,
}

pub struct HashView;

impl HashView {
    pub fn show(
        ui: &mut egui::Ui,
        state: &mut HashViewState,
        hashes: Option<&FileHashes>,
        on_pick_file: impl FnOnce(PathBuf),
    ) {
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("# {}", t("tools.hash_calc"))).size(18.0).strong());
            });
            ui.label(RichText::new(t("tools.hash_calc_desc")).size(12.0).color(ui.visuals().weak_text_color()));
            ui.add_space(10.0);

            // File selection row
            ui.horizontal(|ui| {
                if ui.button(format!("📁 {}", t("tools.pick_file"))).clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        state.selected_file = Some(path.clone());
                        on_pick_file(path);
                    }
                }

                if let Some(p) = &state.selected_file {
                    ui.label(RichText::new(p.display().to_string()).size(12.5).color(ui.visuals().text_color()));
                } else {
                    ui.label(RichText::new(t("tools.no_file")).size(12.0).color(ui.visuals().weak_text_color()));
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);

            if let Some(h) = hashes {
                // Meta header
                ui.horizontal(|ui| {
                    ui.label(format!("{}: {}", t("table.size"), format_bytes(h.file_size)));
                    ui.separator();
                    ui.label(format!("{}: {:.3}s ({:.1} MB/s)", t("tools.time"), h.elapsed_seconds, h.throughput_mb_s));
                });
                ui.add_space(10.0);

                // Hash values table
                let hash_rows = [
                    ("CRC32", &h.crc32),
                    ("MD5", &h.md5),
                    ("SHA-1", &h.sha1),
                    ("SHA-256", &h.sha256),
                    ("SHA-512", &h.sha512),
                    ("BLAKE3", &h.blake3),
                ];

                for (name, val) in hash_rows {
                    ui.horizontal(|ui| {
                        ui.add_sized(Vec2::new(75.0, 22.0), egui::Label::new(RichText::new(name).strong().color(crate::theme::accent_text(ui))));

                        let is_match = !state.compare_input.is_empty() && state.compare_input.trim().eq_ignore_ascii_case(val);
                        let val_color = if is_match {
                            Color32::from_rgb(46, 204, 113)
                        } else {
                            ui.visuals().text_color()
                        };

                        ui.add(
                            egui::Label::new(RichText::new(val).monospace().size(12.0).color(val_color))
                        );

                        if ui.small_button(t("dialogs.hash.copy")).clicked() {
                            ui.ctx().copy_text(val.to_string());
                            state.copied_indicator = Some(name.to_string());
                        }

                        if is_match {
                            ui.label(RichText::new(format!("✓ {}", t("tools.match"))).size(11.0).strong().color(Color32::from_rgb(46, 204, 113)));
                        }
                    });
                    ui.add_space(4.0);
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(10.0);

                // Verification input
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("tools.compare_hash")).strong());
                    ui.add(
                        egui::TextEdit::singleline(&mut state.compare_input)
                            .hint_text(t("tools.compare_hash_hint"))
                            .desired_width(400.0),
                    );
                    if ui.button(t("common.clear")).clicked() {
                        state.compare_input.clear();
                    }
                });
            } else if state.selected_file.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t("status.loading"));
                });
            }
        });
    }
}
