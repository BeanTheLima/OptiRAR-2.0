use eframe::egui::{self, RichText, Vec2};
use optirar_core::i18n::{get_i18n, Language};
use optirar_core::t;
use std::path::Path;

use crate::theme;

#[derive(Default)]
pub struct TitleBarAction {
    pub toggle_theme: bool,
    pub toggle_language: bool,
    pub open_settings: bool,
    pub open_about: bool,
}

pub struct TitleBarView;

impl TitleBarView {
    pub fn show(
        ui: &mut egui::Ui,
        archive_path: Option<&Path>,
        is_solid: bool,
        search_query: &mut String,
        is_dark: bool,
    ) -> TitleBarAction {
        let mut action = TitleBarAction::default();

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            ui.label(RichText::new("📦 OptiRAR").strong().size(16.0).color(theme::accent_text(ui)));

            if let Some(path) = archive_path {
                let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                ui.label(RichText::new(format!("— {}", name)).size(14.0)).on_hover_text(path.display().to_string());
                if is_solid {
                    ui.label(RichText::new("SOLID").size(10.5).strong().color(theme::WARNING));
                }
            } else {
                ui.label(RichText::new(format!("— {}", t("status.ready"))).size(13.0).color(theme::weak_text(ui)));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("⚙").size(15.0)).on_hover_text(t("nav.settings")).clicked() {
                    action.open_settings = true;
                }
                let theme_icon = if is_dark { "☀" } else { "🌙" };
                if ui.button(RichText::new(theme_icon).size(14.0)).on_hover_text(t("settings.appearance")).clicked() {
                    action.toggle_theme = true;
                }
                let lang_label = match get_i18n().language() {
                    Language::EnUs => "EN",
                    Language::ViVn => "VI",
                };
                if ui.button(RichText::new(lang_label).strong()).on_hover_text(t("settings.language")).clicked() {
                    action.toggle_language = true;
                }
                if archive_path.is_some() {
                    ui.add(
                        egui::TextEdit::singleline(search_query)
                            .desired_width(220.0)
                            .hint_text(format!("🔍 {}", t("toolbar.search_hint"))),
                    );
                }
            });
        });

        action
    }
}
