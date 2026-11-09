use eframe::egui::{self, Color32, CornerRadius, Margin, ProgressBar, RichText};
use optirar_core::queue::{TaskQueue, TaskState};
use optirar_core::t;

pub struct QueueView;

impl QueueView {
    pub fn show(
        ui: &mut egui::Ui,
        queue: &TaskQueue,
    ) {
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("⏱ {}", t("nav.task_queue"))).size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(t("queue.clear_completed")).clicked() {
                        queue.clear_completed();
                    }
                });
            });
            ui.add_space(4.0);
            ui.label(RichText::new(t("queue.desc")).size(12.0).color(ui.visuals().weak_text_color()));
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            let items = queue.list();
            if items.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(RichText::new(t("queue.empty")).size(14.0).color(ui.visuals().weak_text_color()));
                });
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for item in items {
                        egui::Frame::NONE
                            .fill(ui.visuals().faint_bg_color)
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("#{}", item.id)).strong().color(crate::theme::accent_text(ui)));
                                    ui.label(RichText::new(&item.title).strong().size(13.0));

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        match &item.state {
                                            TaskState::Running { .. } => {
                                                if ui.small_button(t("dialogs.extract.cancel")).clicked() {
                                                    queue.cancel(item.id);
                                                }
                                                ui.label(RichText::new(t("queue.running")).strong().color(crate::theme::accent_text(ui)));
                                            }
                                            TaskState::Pending => {
                                                if ui.small_button(t("dialogs.extract.cancel")).clicked() {
                                                    queue.cancel(item.id);
                                                }
                                                ui.label(RichText::new(t("queue.pending")).color(Color32::from_rgb(241, 196, 15)));
                                            }
                                            TaskState::Completed { .. } => {
                                                ui.label(RichText::new(t("queue.completed")).strong().color(Color32::from_rgb(46, 204, 113)));
                                            }
                                            TaskState::Failed { .. } => {
                                                ui.label(RichText::new(t("queue.failed")).strong().color(Color32::from_rgb(231, 76, 60)));
                                            }
                                            TaskState::Cancelled => {
                                                ui.label(RichText::new(t("queue.cancelled")).color(ui.visuals().weak_text_color()));
                                            }
                                        }
                                    });
                                });

                                match &item.state {
                                    TaskState::Running { progress, current_file, throughput_mb_s } => {
                                        ui.add_space(6.0);
                                        ui.add(ProgressBar::new(*progress / 100.0).show_percentage());
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("{:.1} MB/s", throughput_mb_s)).size(11.0).color(ui.visuals().weak_text_color()));
                                            ui.label(RichText::new(current_file).size(11.0).color(ui.visuals().weak_text_color()));
                                        });
                                    }
                                    TaskState::Completed { message, elapsed_secs } => {
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(format!("{} ({:.2}s)", message, elapsed_secs)).size(11.5).color(crate::theme::SUCCESS));
                                    }
                                    TaskState::Failed { error } => {
                                        ui.add_space(4.0);
                                        ui.label(RichText::new(error).size(11.5).color(crate::theme::ERROR));
                                    }
                                    _ => {}
                                }
                            });
                        ui.add_space(6.0);
                    }
                });
            }
        });
    }
}
