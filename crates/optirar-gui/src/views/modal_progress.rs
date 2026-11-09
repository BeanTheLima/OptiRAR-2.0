use eframe::egui::{self, Align2, ProgressBar, RichText, Vec2, Window};
use optirar_core::t;
use optirar_core::traits::ProgressSnapshot;
use optirar_core::types::format_bytes;

use crate::theme;

pub struct ProgressModalView;

impl ProgressModalView {
    pub fn show(ctx: &egui::Context, title: &str, progress: Option<&ProgressSnapshot>, on_cancel: impl FnOnce()) {
        Window::new(RichText::new(title).strong().size(15.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(460.0, 0.0))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                match progress {
                    Some(snap) => {
                        let pct = (snap.percent() / 100.0).clamp(0.0, 1.0) as f32;
                        ui.add(ProgressBar::new(pct).show_percentage().fill(theme::ACCENT).desired_height(22.0));
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(RichText::new(&snap.current_file).strong()).truncate());
                        ui.add_space(4.0);
                        egui::Grid::new("progress_stats_grid").num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
                            ui.label(t("dialogs.progress.progress"));
                            ui.label(format!(
                                "{} / {}  ({} / {})",
                                snap.files_completed,
                                snap.total_files,
                                format_bytes(snap.bytes_completed),
                                format_bytes(snap.total_bytes)
                            ));
                            ui.end_row();
                            ui.label(t("dialogs.progress.throughput"));
                            ui.label(RichText::new(snap.display_throughput()).color(theme::SUCCESS).strong());
                            ui.end_row();
                            ui.label(t("dialogs.progress.elapsed_eta"));
                            ui.label(format!("{:.1}s ({})", snap.elapsed_seconds, snap.display_eta()));
                            ui.end_row();
                        });
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t("status.loading"));
                        });
                    }
                }

                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new(t("dialogs.progress.cancel")).color(theme::ERROR)).clicked() {
                        on_cancel();
                    }
                });
            });
    }
}
