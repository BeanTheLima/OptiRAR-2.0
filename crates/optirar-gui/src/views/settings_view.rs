use eframe::egui::{self, RichText};
use optirar_core::config::{AppConfig, ThemeMode};
use optirar_core::i18n::{get_i18n, Language};
use optirar_core::t;
use optirar_core::types::CompressionProfile;

use crate::theme;

#[derive(Default)]
pub struct SettingsAction {
    pub language_changed: Option<Language>,
    pub theme_changed: Option<ThemeMode>,
    pub register_shell_clicked: bool,
    pub unregister_shell_clicked: bool,
    pub save_config_clicked: bool,
    pub codepage_changed: bool,
}

pub struct SettingsView;

impl SettingsView {
    pub fn show(ui: &mut egui::Ui, config: &mut AppConfig, shell_status_msg: Option<&(bool, String)>) -> SettingsAction {
        let mut action = SettingsAction::default();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(720.0);
            ui.add_space(8.0);
            ui.label(RichText::new(format!("⚙ {}", t("settings.title"))).size(20.0).strong());
            ui.add_space(12.0);

            let section = |ui: &mut egui::Ui, key: &str| {
                ui.add_space(6.0);
                ui.label(RichText::new(t(key)).size(15.0).strong().color(theme::accent_text(ui)));
                ui.add_space(4.0);
            };

            section(ui, "settings.language");
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    for lang in [Language::ViVn, Language::EnUs] {
                        if ui.selectable_label(config.language == lang, lang.display_name()).clicked() {
                            config.language = lang;
                            get_i18n().set_language(lang);
                            action.language_changed = Some(lang);
                            action.save_config_clicked = true;
                        }
                    }
                });
            });

            section(ui, "settings.appearance");
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    for (mode, key) in [(ThemeMode::Dark, "settings.theme_dark"), (ThemeMode::Light, "settings.theme_light")] {
                        if ui.selectable_label(config.theme == mode, t(key)).clicked() {
                            config.theme = mode;
                            action.theme_changed = Some(mode);
                            action.save_config_clicked = true;
                        }
                    }
                });
            });

            section(ui, "settings.shell");
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(t("settings.shell_desc")).color(theme::weak_text(ui)));
                ui.add_space(6.0);
                let status_key = if config.shell_integration_enabled {
                    "settings.shell_status_enabled"
                } else {
                    "settings.shell_status_disabled"
                };
                ui.label(RichText::new(t(status_key)).strong());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(RichText::new(t("settings.register_shell")).color(egui::Color32::WHITE)).fill(theme::ACCENT))
                        .clicked()
                    {
                        action.register_shell_clicked = true;
                    }
                    if ui.button(t("settings.unregister_shell")).clicked() {
                        action.unregister_shell_clicked = true;
                    }
                });
                if let Some((ok, msg)) = shell_status_msg {
                    ui.add_space(6.0);
                    ui.label(RichText::new(msg).color(if *ok { theme::SUCCESS } else { theme::ERROR }));
                }
            });

            section(ui, "settings.extraction");
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                action.save_config_clicked |= ui.checkbox(&mut config.smart_extract, t("settings.auto_smart")).changed();
                action.save_config_clicked |= ui
                    .checkbox(&mut config.open_folder_after_extract, t("settings.open_folder_after"))
                    .changed();
                action.save_config_clicked |= ui
                    .checkbox(&mut config.auto_check_integrity, t("settings.auto_integrity"))
                    .changed();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(t("settings.default_dest"));
                    let shown = config
                        .default_destination
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| t("settings.dest_next_to_archive"));
                    ui.label(RichText::new(shown).strong());
                    if ui.button(t("dialogs.extract.browse")).clicked() {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            config.default_destination = Some(dir);
                            action.save_config_clicked = true;
                        }
                    }
                    if config.default_destination.is_some() && ui.button("✖").clicked() {
                        config.default_destination = None;
                        action.save_config_clicked = true;
                    }
                });
            });

            ui.add_space(4.0);
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(t("settings.zip_codepage"));
                    let label = |cp: u32| match cp {
                        0 => t("settings.codepage_auto"),
                        1258 => "1258 — Tiếng Việt (Windows)".to_string(),
                        437 => "437 — DOS (US)".to_string(),
                        850 => "850 — DOS (Western Europe)".to_string(),
                        1252 => "1252 — Windows (Western)".to_string(),
                        936 => "936 — 简体中文 (GBK)".to_string(),
                        932 => "932 — 日本語 (Shift-JIS)".to_string(),
                        949 => "949 — 한국어".to_string(),
                        other => other.to_string(),
                    };
                    egui::ComboBox::from_id_salt("zip_codepage")
                        .width(260.0)
                        .selected_text(label(config.zip_name_codepage))
                        .show_ui(ui, |ui| {
                            for cp in [0, 1258, 437, 850, 1252, 936, 932, 949] {
                                if ui.selectable_value(&mut config.zip_name_codepage, cp, label(cp)).clicked() {
                                    action.codepage_changed = true;
                                    action.save_config_clicked = true;
                                }
                            }
                        });
                });
                ui.label(RichText::new(t("settings.zip_codepage_desc")).size(12.0).color(theme::weak_text(ui)));
            });

            section(ui, "settings.performance");
            theme::card(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (p, key) in [
                    (CompressionProfile::Eco, "settings.eco_desc"),
                    (CompressionProfile::Balanced, "settings.balanced_desc"),
                    (CompressionProfile::Performance, "settings.perf_desc"),
                ] {
                    if ui.radio(config.default_compression_profile == p, t(key)).clicked() {
                        config.default_compression_profile = p;
                        action.save_config_clicked = true;
                    }
                }
            });

            ui.add_space(20.0);
        });

        action
    }
}
