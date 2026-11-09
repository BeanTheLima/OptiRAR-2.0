use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke, Vec2};
use optirar_core::t;
use std::path::PathBuf;

use crate::theme;

#[derive(Default)]
pub struct WelcomeAction {
    pub open_clicked: bool,
    pub create_clicked: bool,
    pub hash_clicked: bool,
    pub extract_many_clicked: bool,
    pub open_recent: Option<PathBuf>,
    pub remove_recent: Option<PathBuf>,
}

pub struct WelcomeView;

impl WelcomeView {
    pub fn show(ui: &mut egui::Ui, recent_archives: &[PathBuf]) -> WelcomeAction {
        let mut action = WelcomeAction::default();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(28.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("📦 OptiRAR").size(30.0).strong().color(theme::accent_text(ui)));
                ui.add_space(2.0);
                ui.label(RichText::new(t("app.tagline")).size(14.0).color(theme::weak_text(ui)));
            });
            ui.add_space(22.0);

            ui.vertical_centered(|ui| {
                egui::Frame::NONE
                    .fill(ui.visuals().faint_bg_color)
                    .stroke(Stroke::new(1.5_f32, theme::ACCENT))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::same(24))
                    .show(ui, |ui| {
                        ui.set_max_width(540.0);
                        ui.label(RichText::new("📥").size(36.0).color(theme::accent_text(ui)));
                        ui.add_space(6.0);
                        ui.label(RichText::new(t("table.drag_drop_zone")).size(15.0).strong());
                        ui.label(RichText::new(t("table.no_archive_subtitle")).size(12.5).color(theme::weak_text(ui)));
                        ui.add_space(16.0);

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
                            let big = |ui: &mut egui::Ui, text: String, fill: Color32, fg: Color32| {
                                ui.add_sized(
                                    Vec2::new(165.0, 38.0),
                                    egui::Button::new(RichText::new(text).strong().size(14.0).color(fg))
                                        .fill(fill)
                                        .corner_radius(CornerRadius::same(6)),
                                )
                                .clicked()
                            };
                            action.open_clicked = big(ui, format!("📂 {}", t("nav.open_archive")), theme::ACCENT, Color32::WHITE);
                            action.create_clicked = big(ui, format!("➕ {}", t("dialogs.create.title")), Color32::from_rgb(46, 125, 50), Color32::WHITE);
                            let fill = ui.visuals().widgets.inactive.bg_fill;
                            let fg = ui.visuals().text_color();
                            action.hash_clicked = big(ui, format!("# {}", t("nav.hash_utility")), fill, fg);
                        });
                        ui.add_space(8.0);
                        if ui.link(format!("📤 {}", t("welcome.extract_many"))).on_hover_text(t("welcome.extract_many_tip")).clicked() {
                            action.extract_many_clicked = true;
                        }
                    });
            });

            ui.add_space(22.0);

            if !recent_archives.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(540.0);
                    ui.label(RichText::new(t("nav.recent_archives")).size(14.0).strong());
                    ui.add_space(4.0);

                    for path in recent_archives.iter().take(8) {
                        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let exists = path.exists();
                        let resp = theme::card(ui)
                            .inner_margin(Margin::symmetric(12, 6))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("📦").size(16.0));
                                    ui.vertical(|ui| {
                                        let mut title = RichText::new(&name).size(13.5).strong();
                                        if !exists {
                                            title = title.strikethrough().color(theme::weak_text(ui));
                                        }
                                        ui.label(title);
                                        ui.label(RichText::new(path.display().to_string()).size(11.0).color(theme::weak_text(ui)));
                                    });
                                });
                            })
                            .response
                            .interact(egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if resp.clicked() && exists {
                            action.open_recent = Some(path.clone());
                        }
                        resp.context_menu(|ui| {
                            if ui.button(t("ctx.remove_recent")).clicked() {
                                action.remove_recent = Some(path.clone());
                                ui.close_menu();
                            }
                        });
                        ui.add_space(4.0);
                    }
                });
                ui.add_space(18.0);
            }

            ui.vertical_centered(|ui| {
                ui.label(RichText::new(t("table.supported_formats")).size(12.0).color(theme::weak_text(ui)));
            });
            ui.add_space(24.0);
        });

        action
    }
}
