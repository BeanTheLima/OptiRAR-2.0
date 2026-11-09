use eframe::egui::{self, Align2, ComboBox, RichText, Vec2, Window};
use optirar_core::i18n::t_format;
use optirar_core::t;
use optirar_core::types::{format_bytes, ArchiveFormat, CompressionLevel, CompressionProfile, OverwritePolicy};
use optirar_index::catalog::ArchiveCatalog;
use std::path::PathBuf;

use crate::theme;

pub struct CreateDialogState {
    pub is_open: bool,
    pub destination: PathBuf,
    pub sources: Vec<PathBuf>,
    pub format: ArchiveFormat,
    pub level: CompressionLevel,
    pub password: String,
    pub show_password: bool,
}

impl Default for CreateDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            destination: PathBuf::new(),
            sources: Vec::new(),
            format: ArchiveFormat::Zip,
            level: CompressionLevel::Normal,
            password: String::new(),
            show_password: false,
        }
    }
}

impl CreateDialogState {
    /// Open the dialog for `sources`, proposing "<name>.zip" next to them (WinRAR style).
    pub fn open_with(&mut self, sources: Vec<PathBuf>) {
        if let Some(first) = sources.first() {
            self.destination = default_archive_name(&sources, first, self.format);
        }
        self.sources = sources;
        self.is_open = true;
    }
}

pub fn format_ext(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Tar => "tar",
        ArchiveFormat::Gzip => "gz",
        ArchiveFormat::Zstd => "zst",
        ArchiveFormat::SevenZip => "7z",
        _ => "zip",
    }
}

/// "<file stem | folder name | parent folder name>.<ext>" next to the sources, never overwriting.
pub fn default_archive_name(sources: &[PathBuf], first: &PathBuf, format: ArchiveFormat) -> PathBuf {
    let parent = first.parent().map(PathBuf::from).unwrap_or_default();
    let base = if sources.len() == 1 {
        if first.is_dir() { first.file_name() } else { first.file_stem() }
    } else {
        parent.file_name()
    }
    .map(|s| s.to_string_lossy().to_string())
    .unwrap_or_else(|| "Archive".into());

    let ext = format_ext(format);
    let mut candidate = parent.join(format!("{}.{}", base, ext));
    let mut n = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{} ({}).{}", base, n, ext));
        n += 1;
    }
    candidate
}

pub struct ExtractDialogState {
    pub is_open: bool,
    pub destination: PathBuf,
    pub profile: CompressionProfile,
    pub overwrite: OverwritePolicy,
    pub extract_selected_only: bool,
}

impl Default for ExtractDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            destination: PathBuf::new(),
            profile: CompressionProfile::Balanced,
            overwrite: OverwritePolicy::Overwrite,
            extract_selected_only: false,
        }
    }
}

#[derive(Default)]
pub struct PasswordDialogState {
    pub is_open: bool,
    pub input: String,
    pub show: bool,
}

fn overwrite_label(p: OverwritePolicy) -> String {
    t(match p {
        OverwritePolicy::Overwrite => "dialogs.extract.ow_overwrite",
        OverwritePolicy::SkipExisting => "dialogs.extract.ow_skip",
        OverwritePolicy::RenameNew => "dialogs.extract.ow_rename",
        OverwritePolicy::ErrorOnConflict => "dialogs.extract.ow_error",
    })
}

fn modal(title: String) -> Window<'static> {
    Window::new(RichText::new(title).strong().size(15.0))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .collapsible(false)
        .resizable(false)
}

pub struct DialogsView;

impl DialogsView {
    pub fn show_create_dialog(
        ctx: &egui::Context,
        state: &mut CreateDialogState,
        on_create: impl FnOnce(PathBuf, Vec<PathBuf>, ArchiveFormat, CompressionLevel, Option<String>),
    ) {
        if !state.is_open {
            return;
        }
        let mut start_create = false;

        modal(t("dialogs.create.title")).fixed_size(Vec2::new(560.0, 0.0)).show(ctx, |ui| {
            ui.label(RichText::new(t("dialogs.create.destination")).strong());
            ui.horizontal(|ui| {
                let mut path_str = state.destination.to_string_lossy().to_string();
                if ui.add(egui::TextEdit::singleline(&mut path_str).desired_width(440.0)).changed() {
                    state.destination = PathBuf::from(path_str);
                }
                if ui.button(t("dialogs.create.browse")).clicked() {
                    let ext = format_ext(state.format);
                    let mut dlg = rfd::FileDialog::new().add_filter("Archive", &[ext]);
                    if let Some(name) = state.destination.file_name() {
                        dlg = dlg.set_file_name(name.to_string_lossy());
                    }
                    if let Some(dir) = state.destination.parent().filter(|d| d.is_dir()) {
                        dlg = dlg.set_directory(dir);
                    }
                    if let Some(path) = dlg.save_file() {
                        state.destination = path;
                    }
                }
            });
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(t("dialogs.create.format"));
                let before = state.format;
                ComboBox::from_id_salt("create_format_combo")
                    .selected_text(format_ext(state.format).to_uppercase())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.format, ArchiveFormat::Zip, "ZIP (.zip)");
                        ui.selectable_value(&mut state.format, ArchiveFormat::SevenZip, "7-Zip (.7z)");
                        ui.selectable_value(&mut state.format, ArchiveFormat::Tar, "TAR (.tar)");
                        ui.selectable_value(&mut state.format, ArchiveFormat::Gzip, "GZIP (.gz)");
                        ui.selectable_value(&mut state.format, ArchiveFormat::Zstd, "Zstandard (.zst)");
                    });
                if before != state.format && !state.destination.as_os_str().is_empty() {
                    state.destination.set_extension(format_ext(state.format));
                }

                ui.add_space(16.0);
                ui.label(t("dialogs.create.level"));
                let level_label = |l: CompressionLevel| {
                    t(match l {
                        CompressionLevel::Store => "dialogs.create.level_store",
                        CompressionLevel::Fastest => "dialogs.create.level_fastest",
                        CompressionLevel::Normal => "dialogs.create.level_normal",
                        CompressionLevel::Maximum => "dialogs.create.level_max",
                    })
                };
                ComboBox::from_id_salt("create_level_combo")
                    .selected_text(level_label(state.level))
                    .show_ui(ui, |ui| {
                        for l in [CompressionLevel::Store, CompressionLevel::Fastest, CompressionLevel::Normal, CompressionLevel::Maximum] {
                            ui.selectable_value(&mut state.level, l, level_label(l));
                        }
                    });
            });

            ui.add_space(6.0);
            let encryptable = matches!(state.format, ArchiveFormat::Zip | ArchiveFormat::SevenZip);
            ui.horizontal(|ui| {
                ui.label(format!("🔒 {}", t("dialogs.create.password")));
                ui.add_enabled(
                    encryptable,
                    egui::TextEdit::singleline(&mut state.password).password(!state.show_password).desired_width(220.0).hint_text(t("dialogs.create.password_hint")),
                );
                ui.add_enabled(encryptable, egui::Checkbox::new(&mut state.show_password, t("dialogs.password.show")));
            });
            if !encryptable && !state.password.is_empty() {
                ui.label(RichText::new(t("dialogs.create.password_unsupported")).size(11.5).color(theme::WARNING));
            }
            ui.add_space(4.0);
            ui.label(RichText::new(t("dialogs.create.roshal_notice")).italics().size(11.5).color(theme::weak_text(ui)));
            ui.add_space(10.0);

            ui.label(RichText::new(t_format("dialogs.create.sources", &[("count", &state.sources.len().to_string())])).strong());
            ui.horizontal(|ui| {
                if ui.button(t("dialogs.create.add_files")).clicked() {
                    if let Some(files) = rfd::FileDialog::new().pick_files() {
                        state.sources.extend(files);
                    }
                }
                if ui.button(t("dialogs.create.add_folder")).clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        state.sources.push(folder);
                    }
                }
                if ui.button(t("dialogs.create.clear_sources")).clicked() {
                    state.sources.clear();
                }
            });

            let mut remove = None;
            egui::Frame::NONE.fill(ui.visuals().extreme_bg_color).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(120.0).auto_shrink([false, true]).show(ui, |ui| {
                    for (idx, src) in state.sources.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.small_button("✖").clicked() {
                                remove = Some(idx);
                            }
                            let icon = if src.is_dir() { "📁" } else { "📄" };
                            ui.label(format!("{} {}", icon, src.display()));
                        });
                    }
                });
            });
            if let Some(i) = remove {
                state.sources.remove(i);
            }

            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t("dialogs.create.cancel")).clicked() {
                    state.is_open = false;
                }
                let can_create = !state.destination.as_os_str().is_empty() && !state.sources.is_empty();
                if ui
                    .add_enabled(
                        can_create,
                        egui::Button::new(RichText::new(t("dialogs.create.create_btn")).strong().color(egui::Color32::WHITE)).fill(theme::ACCENT),
                    )
                    .clicked()
                {
                    start_create = true;
                }
            });
        });

        if start_create {
            state.is_open = false;
            let pw = matches!(state.format, ArchiveFormat::Zip | ArchiveFormat::SevenZip).then(|| state.password.clone()).filter(|p| !p.is_empty());
            state.password.clear();
            on_create(state.destination.clone(), state.sources.clone(), state.format, state.level, pw);
        }
    }

    pub fn show_extract_dialog(
        ctx: &egui::Context,
        state: &mut ExtractDialogState,
        selected_count: usize,
        smart_extract: &mut bool,
        open_after: &mut bool,
        on_extract: impl FnOnce(PathBuf, CompressionProfile, OverwritePolicy),
    ) {
        if !state.is_open {
            return;
        }
        let mut start_extract = false;

        let title = if state.extract_selected_only {
            t_format("dialogs.extract.title_selected", &[("count", &selected_count.to_string())])
        } else {
            t("dialogs.extract.title_all")
        };

        modal(title).fixed_size(Vec2::new(540.0, 0.0)).show(ctx, |ui| {
            ui.label(RichText::new(t("dialogs.extract.destination")).strong());
            ui.horizontal(|ui| {
                let mut path_str = state.destination.to_string_lossy().to_string();
                if ui.add(egui::TextEdit::singleline(&mut path_str).desired_width(420.0)).changed() {
                    state.destination = PathBuf::from(path_str);
                }
                if ui.button(t("dialogs.extract.browse")).clicked() {
                    let mut dlg = rfd::FileDialog::new();
                    if let Some(dir) = state.destination.ancestors().find(|d| d.is_dir()) {
                        dlg = dlg.set_directory(dir);
                    }
                    if let Some(path) = dlg.pick_folder() {
                        state.destination = path;
                    }
                }
            });
            ui.add_space(8.0);

            egui::Grid::new("extract_opts").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                ui.label(t("dialogs.extract.overwrite"));
                ComboBox::from_id_salt("extract_overwrite_combo")
                    .selected_text(overwrite_label(state.overwrite))
                    .show_ui(ui, |ui| {
                        for p in [OverwritePolicy::Overwrite, OverwritePolicy::SkipExisting, OverwritePolicy::RenameNew] {
                            ui.selectable_value(&mut state.overwrite, p, overwrite_label(p));
                        }
                    });
                ui.end_row();

                ui.label(t("dialogs.extract.profile"));
                let profile_label = |p: CompressionProfile| match p {
                    CompressionProfile::Eco => t("settings.eco_desc"),
                    CompressionProfile::Performance => t("settings.perf_desc"),
                    _ => t("settings.balanced_desc"),
                };
                ComboBox::from_id_salt("extract_profile_combo")
                    .width(320.0)
                    .selected_text(profile_label(state.profile))
                    .show_ui(ui, |ui| {
                        for p in [CompressionProfile::Eco, CompressionProfile::Balanced, CompressionProfile::Performance] {
                            ui.selectable_value(&mut state.profile, p, profile_label(p));
                        }
                    });
                ui.end_row();
            });

            ui.add_space(6.0);
            if !state.extract_selected_only {
                ui.checkbox(smart_extract, t("dialogs.extract.smart_extract"));
            }
            ui.checkbox(open_after, t("settings.open_folder_after"));

            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t("dialogs.extract.cancel")).clicked() {
                    state.is_open = false;
                }
                let can = !state.destination.as_os_str().is_empty();
                if ui
                    .add_enabled(
                        can,
                        egui::Button::new(RichText::new(t("dialogs.extract.start")).strong().color(egui::Color32::WHITE)).fill(theme::ACCENT),
                    )
                    .clicked()
                {
                    start_extract = true;
                }
            });
        });

        if start_extract {
            state.is_open = false;
            on_extract(state.destination.clone(), state.profile, state.overwrite);
        }
    }

    /// Returns Some(password) when the user confirms, Some(None) is never produced; cancel closes it.
    pub fn show_password_dialog(ctx: &egui::Context, state: &mut PasswordDialogState) -> Option<String> {
        if !state.is_open {
            return None;
        }
        let mut result = None;
        modal(format!("🔒 {}", t("dialogs.password.title"))).fixed_size(Vec2::new(400.0, 0.0)).show(ctx, |ui| {
            ui.label(t("dialogs.password.prompt"));
            ui.add_space(6.0);
            let resp = ui.add(
                egui::TextEdit::singleline(&mut state.input)
                    .password(!state.show)
                    .desired_width(f32::INFINITY),
            );
            resp.request_focus();
            ui.checkbox(&mut state.show, t("dialogs.password.show"));
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t("dialogs.extract.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    state.is_open = false;
                }
                if ui
                    .add(egui::Button::new(RichText::new("OK").strong().color(egui::Color32::WHITE)).fill(theme::ACCENT))
                    .clicked()
                    || enter
                {
                    result = Some(state.input.clone());
                    state.is_open = false;
                }
            });
        });
        result
    }

    pub fn show_properties_dialog(ctx: &egui::Context, is_open: &mut bool, catalog: Option<&ArchiveCatalog>, archive_path: Option<&std::path::Path>) {
        if !*is_open {
            return;
        }

        modal(t("dialogs.properties.title")).fixed_size(Vec2::new(480.0, 0.0)).show(ctx, |ui| {
            if let Some(cat) = catalog {
                let yes_no = |b: bool| if b { t("common.yes") } else { t("common.no") };
                egui::Grid::new("archive_props_grid").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                    let mut row = |k: &str, v: String| {
                        ui.label(RichText::new(t(k)).strong());
                        ui.label(v);
                        ui.end_row();
                    };
                    if let Some(p) = archive_path {
                        row("dialogs.properties.location", p.display().to_string());
                        if let Ok(meta) = std::fs::metadata(p) {
                            row("dialogs.properties.compressed_size", format!("{} ({} bytes)", format_bytes(meta.len()), meta.len()));
                        }
                    }
                    row("dialogs.properties.format", format!("{:?}", cat.format));
                    row("dialogs.properties.solid", yes_no(cat.is_solid));
                    row("dialogs.properties.encryption", yes_no(cat.entries.iter().any(|e| e.is_encrypted)));
                    row("dialogs.properties.total_files", cat.file_count.to_string());
                    row("dialogs.properties.total_dirs", cat.dir_count.to_string());
                    row(
                        "dialogs.properties.uncompressed_size",
                        format!("{} ({} bytes)", format_bytes(cat.total_uncompressed_bytes), cat.total_uncompressed_bytes),
                    );
                    let ratio = if cat.total_uncompressed_bytes > 0 {
                        cat.total_compressed_bytes as f64 / cat.total_uncompressed_bytes as f64 * 100.0
                    } else {
                        100.0
                    };
                    row("dialogs.properties.ratio", format!("{:.2}%", ratio));
                });
            } else {
                ui.label(t("table.no_archive_title"));
            }

            ui.add_space(10.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t("dialogs.properties.close")).clicked() {
                    *is_open = false;
                }
            });
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropChoice {
    AddToArchive,
    NewArchive,
    Cancel,
}

/// Files dropped while a ZIP is open: add them to it, or make a new archive.
pub fn drop_choice_dialog(ctx: &egui::Context, count: usize, archive_name: &str, folder: &str) -> Option<DropChoice> {
    let mut choice = None;
    modal(t("dialogs.drop.title")).fixed_size(Vec2::new(460.0, 0.0)).show(ctx, |ui| {
        ui.label(t_format("dialogs.drop.prompt", &[("count", &count.to_string())]));
        ui.add_space(8.0);
        let target = if folder.is_empty() { archive_name.to_string() } else { format!("{}/{}", archive_name, folder.trim_end_matches('/')) };
        if ui.add(egui::Button::new(RichText::new(format!("➕ {}", t_format("dialogs.drop.add", &[("target", &target)]))).strong()).min_size(Vec2::new(420.0, 30.0))).clicked() {
            choice = Some(DropChoice::AddToArchive);
        }
        if ui.add(egui::Button::new(format!("📦 {}", t("dialogs.drop.new"))).min_size(Vec2::new(420.0, 30.0))).clicked() {
            choice = Some(DropChoice::NewArchive);
        }
        ui.add_space(6.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(t("dialogs.extract.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                choice = Some(DropChoice::Cancel);
            }
        });
    });
    choice
}

/// Yes/no confirmation. Some(true) = confirmed, Some(false) = cancelled.
pub fn confirm_dialog(ctx: &egui::Context, title: &str, message: &str, ok_label: &str) -> Option<bool> {
    let mut result = None;
    modal(title.to_string()).fixed_size(Vec2::new(420.0, 0.0)).show(ctx, |ui| {
        ui.label(message);
        ui.add_space(10.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(t("dialogs.extract.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                result = Some(false);
            }
            if ui.add(egui::Button::new(RichText::new(ok_label).strong().color(egui::Color32::WHITE)).fill(theme::ERROR)).clicked() {
                result = Some(true);
            }
        });
    });
    result
}
