use eframe::egui::{self, CentralPanel, Key, Modifiers, RichText, SidePanel, TopBottomPanel, ViewportCommand};
use optirar_core::config::{AppConfig, ThemeMode};
use optirar_core::i18n::{get_i18n, t_format, Language};
use optirar_core::queue::{TaskKind, TaskQueue, TaskState};
use optirar_core::t;
use optirar_core::traits::{CancellationToken, ProgressSnapshot};
use optirar_core::types::{ArchiveFormat, CompressionLevel, CompressionProfile, ExtractionOptions, OverwritePolicy};
use optirar_engine::extractor::OptiExtractor;
use optirar_engine::ArchiveDiffResult;
use optirar_index::catalog::ArchiveCatalog;
use optirar_integrity::FileHashes;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::shell;
use crate::theme::{self, configure_visuals};
use crate::views::compare_view::{CompareView, CompareViewState};
use crate::taskbar::{Taskbar, TaskbarState};
use crate::views::dialogs::{
    confirm_dialog, default_archive_name, drop_choice_dialog, CreateDialogState, DialogsView, DropChoice, ExtractDialogState,
    PasswordDialogState,
};
use crate::views::duplicates_view::DuplicatesView;
use crate::views::hash_view::{HashView, HashViewState};
use crate::views::modal_progress::ProgressModalView;
use crate::views::queue_view::QueueView;
use crate::views::settings_view::SettingsView;
use crate::views::sidebar::{NavTab, SidebarView};
use crate::views::status_bar::StatusBarView;
use crate::views::table::{TableState, TableView};
use crate::views::title_bar::TitleBarView;
use crate::views::toolbar::ToolbarView;
use crate::views::viewer::{self, ViewerState};
use crate::views::welcome::WelcomeView;
use crate::worker::{AddItem, GuiProgressListener, WorkerCommand, WorkerEvent, WorkerHandle, PASSWORD_REQUIRED};

const ARCHIVE_EXTS: &[&str] = &["zip", "rar", "7z", "tar", "gz", "tgz", "zst", "tzst", "cbz", "cbr", "cb7"];

/// One-shot actions launched from the Explorer context menu (window closes when done, like WinRAR).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAction {
    ExtractHere,
    ExtractToFolder,
    Test,
    AddZip,
}

#[derive(Debug, Clone, Default)]
pub struct LaunchArgs {
    pub files: Vec<PathBuf>,
    pub open_extract_dialog: bool,
    pub open_create_dialog: bool,
    pub quick: Option<QuickAction>,
}

pub struct OptiRarApp {
    pub config: AppConfig,
    pub archive_path: Option<PathBuf>,
    pub catalog: Option<ArchiveCatalog>,
    pub table: TableState,
    pub search_query: String,
    pub selected_indices: HashSet<usize>,
    pub status_message: String,
    pub is_error_status: bool,

    pub current_tab: NavTab,
    pub sidebar_collapsed: bool,
    pub tools_subtab: usize,

    pub queue: TaskQueue,
    task_ids: VecDeque<(u64, Instant)>,
    pub worker: WorkerHandle,
    pub is_busy: bool,
    pub busy_title: String,
    pub current_progress: Option<ProgressSnapshot>,

    pub create_dialog: CreateDialogState,
    pub extract_dialog: ExtractDialogState,
    password_dialog: PasswordDialogState,
    pub show_properties: bool,
    viewer: Option<ViewerState>,
    /// Files dropped on an open ZIP: ask "add to it" or "new archive".
    pending_drop: Option<Vec<PathBuf>>,
    /// Entries waiting for delete confirmation.
    confirm_delete: Option<Vec<usize>>,
    /// Folder to return to after the archive is re-read (after add/delete/save).
    restore_nav: Option<String>,
    taskbar: Taskbar,
    last_failed: bool,

    pub hash_state: HashViewState,
    pub file_hashes: Option<FileHashes>,
    pub compare_state: CompareViewState,
    pub archive_diff: Option<ArchiveDiffResult>,
    pub is_comparing: bool,
    shell_status_msg: Option<(bool, String)>,

    pub dark_mode: bool,

    password: Option<String>,
    /// Last password-sensitive command, re-sent after the user types a password.
    last_command: Option<WorkerCommand>,
    pending_open: Option<PathBuf>,
    reveal_after: bool,
    quick: Option<QuickAction>,
    quick_pending: usize,
    quick_errors: Vec<String>,
    inject_release: bool,
    window_size: Option<egui::Vec2>,
    fitted_to_screen: bool,
}

fn is_archive(path: &Path) -> bool {
    path.is_file()
        && (ArchiveFormat::from_path(path) != ArchiveFormat::Unknown
            || path
                .extension()
                .map(|e| ARCHIVE_EXTS.contains(&e.to_string_lossy().to_lowercase().as_str()))
                .unwrap_or(false))
}

fn stem_of(path: &Path) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let lower = name.to_lowercase();
    for ext in [".tar.gz", ".tar.zst"] {
        if lower.ends_with(ext) {
            return name[..name.len() - ext.len()].to_string();
        }
    }
    path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Extracted".into())
}

fn parent_dir(path: &Path) -> PathBuf {
    path.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn pick_archive() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter(t("dialogs.open_filter"), ARCHIVE_EXTS)
        .add_filter("*", &["*"])
        .pick_file()
}

fn with_password(cmd: &WorkerCommand, pw: &str) -> WorkerCommand {
    let mut cmd = cmd.clone();
    match &mut cmd {
        WorkerCommand::OpenArchive { password, .. }
        | WorkerCommand::Extract { password, .. }
        | WorkerCommand::TestArchive { password, .. }
        | WorkerCommand::LoadEntry { password, .. } => *password = Some(pw.to_string()),
        _ => {}
    }
    cmd
}

impl OptiRarApp {
    pub fn new(cc: &eframe::CreationContext<'_>, args: LaunchArgs) -> Self {
        let config = AppConfig::load();
        get_i18n().set_language(config.language);

        let dark_mode = !matches!(config.theme, ThemeMode::Light);
        theme::install_fonts(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        optirar_formats::zip_format::set_name_codepage(config.zip_name_codepage);
        configure_visuals(&cc.egui_ctx, dark_mode);

        let mut app = Self {
            config,
            archive_path: None,
            catalog: None,
            table: TableState::default(),
            search_query: String::new(),
            selected_indices: HashSet::new(),
            status_message: String::new(),
            is_error_status: false,
            current_tab: NavTab::Home,
            sidebar_collapsed: false,
            tools_subtab: 0,
            queue: TaskQueue::new(),
            task_ids: VecDeque::new(),
            worker: WorkerHandle::spawn(cc.egui_ctx.clone()),
            is_busy: false,
            busy_title: String::new(),
            current_progress: None,
            create_dialog: CreateDialogState::default(),
            extract_dialog: ExtractDialogState::default(),
            password_dialog: PasswordDialogState::default(),
            show_properties: false,
            viewer: None,
            pending_drop: None,
            confirm_delete: None,
            restore_nav: None,
            taskbar: Taskbar::default(),
            last_failed: false,
            hash_state: HashViewState::default(),
            file_hashes: None,
            compare_state: CompareViewState::default(),
            archive_diff: None,
            is_comparing: false,
            shell_status_msg: None,
            dark_mode,
            password: None,
            last_command: None,
            pending_open: None,
            reveal_after: false,
            quick: args.quick,
            quick_pending: 0,
            quick_errors: Vec::new(),
            inject_release: false,
            window_size: None,
            fitted_to_screen: false,
        };

        if let Some(action) = args.quick {
            app.run_quick_action(action, args.files);
        } else if args.open_create_dialog {
            app.create_dialog.open_with(args.files);
        } else if let Some(path) = args.files.into_iter().next() {
            if is_archive(&path) || !path.is_dir() {
                app.open_archive(path);
                if args.open_extract_dialog {
                    app.extract_dialog.extract_selected_only = false;
                    app.extract_dialog.is_open = true;
                }
            } else {
                app.create_dialog.open_with(vec![path]);
            }
        }

        app
    }

    // ---------------------------------------------------------------- tasks

    fn begin_task(&mut self, kind: TaskKind, title: String) {
        let id = self.queue.enqueue(kind);
        self.task_ids.push_back((id, Instant::now()));
        self.is_busy = true;
        self.busy_title = title;
        self.current_progress = None;
        self.is_error_status = false;
    }

    fn finish_task(&mut self, result: Result<&str, &str>) {
        if let Some((id, started)) = self.task_ids.pop_front() {
            let state = match result {
                Ok(msg) => TaskState::Completed { message: msg.to_string(), elapsed_secs: started.elapsed().as_secs_f64() },
                Err(e) if e == optirar_core::error::OptiError::OperationCancelled.to_string() => TaskState::Cancelled,
                Err(e) => TaskState::Failed { error: e.to_string() },
            };
            self.queue.update_state(id, state);
        }
        self.is_busy = !self.task_ids.is_empty();
        self.current_progress = None;
    }

    fn send(&mut self, cmd: WorkerCommand) {
        if matches!(
            cmd,
            WorkerCommand::OpenArchive { .. } | WorkerCommand::Extract { .. } | WorkerCommand::TestArchive { .. } | WorkerCommand::LoadEntry { .. }
        ) {
            self.last_command = Some(cmd.clone());
        }
        self.worker.send_command(cmd);
    }

    fn run_quick_action(&mut self, action: QuickAction, files: Vec<PathBuf>) {
        if files.is_empty() {
            self.quick_errors.push(t("quick.no_files"));
            return;
        }
        match action {
            QuickAction::AddZip => {
                let dest = default_archive_name(&files, &files[0], ArchiveFormat::Zip);
                self.quick_pending = 1;
                self.start_create(dest, files, ArchiveFormat::Zip, CompressionLevel::Normal, None);
            }
            QuickAction::Test => {
                for f in files {
                    self.quick_pending += 1;
                    self.start_test_on(f);
                }
            }
            QuickAction::ExtractHere | QuickAction::ExtractToFolder => {
                for f in files {
                    let out = if action == QuickAction::ExtractHere { parent_dir(&f) } else { parent_dir(&f).join(stem_of(&f)) };
                    self.quick_pending += 1;
                    self.start_extract_on(f, out, None, false, CompressionProfile::Balanced, OverwritePolicy::Overwrite);
                }
            }
        }
    }

    pub fn open_archive(&mut self, path: PathBuf) {
        self.config.add_recent(&path);
        let _ = self.config.save();

        self.password = None;
        self.catalog = None;
        self.table.reset();
        self.selected_indices.clear();
        self.search_query.clear();
        self.archive_path = Some(path.clone());
        self.current_tab = NavTab::Explorer;
        self.extract_dialog.destination = self.default_extract_dir(&path);
        self.status_message = t("status.loading");
        self.is_busy = true;
        self.busy_title = t_format("msg.opening", &[("name", &stem_of(&path))]);
        self.current_progress = None;
        self.send(WorkerCommand::OpenArchive { path, password: None });
    }

    fn default_extract_dir(&self, archive: &Path) -> PathBuf {
        self.config
            .default_destination
            .clone()
            .unwrap_or_else(|| parent_dir(archive))
            .join(stem_of(archive))
    }

    /// Selected catalog indices (None = whole archive) plus the folder prefix to strip.
    fn selection_for_extract(&self, selected_only: bool) -> (Option<Vec<usize>>, Option<String>) {
        if !selected_only || self.selected_indices.is_empty() {
            return (None, None);
        }
        let mut v: Vec<usize> = self.selected_indices.iter().copied().collect();
        v.sort_unstable();
        let strip = (!self.table.is_searching() && !self.table.nav_path.is_empty()).then(|| self.table.nav_path.clone());
        (Some(v), strip)
    }

    pub fn start_extract(
        &mut self,
        output_dir: PathBuf,
        selection: Option<(Vec<usize>, Option<String>)>,
        smart: bool,
        profile: CompressionProfile,
        overwrite: OverwritePolicy,
    ) {
        let Some(archive_path) = self.archive_path.clone() else { return };
        self.start_extract_on(archive_path, output_dir, selection, smart, profile, overwrite);
    }

    fn start_extract_on(
        &mut self,
        archive_path: PathBuf,
        output_dir: PathBuf,
        selection: Option<(Vec<usize>, Option<String>)>,
        smart: bool,
        profile: CompressionProfile,
        overwrite: OverwritePolicy,
    ) {
        let (selected_indices, strip_prefix) = match selection {
            Some((v, s)) => (Some(v), s),
            None => (None, None),
        };
        self.begin_task(
            TaskKind::Extract { archive: archive_path.clone(), destination: output_dir.clone(), smart },
            t_format("msg.extracting", &[("name", &stem_of(&archive_path))]),
        );
        self.status_message = t("status.extracting");
        self.send(WorkerCommand::Extract {
            archive_path,
            output_dir,
            selected_indices,
            strip_prefix,
            password: self.password.clone(),
            profile,
            overwrite,
            smart,
        });
    }

    fn extract_selected_to(&mut self, output_dir: PathBuf, open_after: bool) {
        let (sel, strip) = self.selection_for_extract(true);
        self.reveal_after = open_after;
        let profile = self.config.default_compression_profile;
        self.start_extract(output_dir, sel.map(|v| (v, strip)), false, profile, OverwritePolicy::Overwrite);
    }

    pub fn start_test(&mut self) {
        if let Some(p) = self.archive_path.clone() {
            self.start_test_on(p);
        }
    }

    fn start_test_on(&mut self, archive_path: PathBuf) {
        self.begin_task(
            TaskKind::Test { archive: archive_path.clone() },
            t_format("msg.testing", &[("name", &stem_of(&archive_path))]),
        );
        self.status_message = t("status.testing");
        self.send(WorkerCommand::TestArchive { archive_path, password: self.password.clone() });
    }

    pub fn start_create(&mut self, destination: PathBuf, sources: Vec<PathBuf>, format: ArchiveFormat, level: CompressionLevel, password: Option<String>) {
        self.begin_task(
            TaskKind::Create {
                output: destination.clone(),
                sources: sources.clone(),
                format,
                profile: self.config.default_compression_profile,
            },
            t_format("msg.creating", &[("name", &destination.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())]),
        );
        self.status_message = t("status.creating");
        self.send(WorkerCommand::CreateArchive { destination, sources, format, level, password });
    }

    /// Load an entry into the built-in viewer/editor.
    pub fn open_viewer(&mut self, index: usize) {
        let Some(archive_path) = self.archive_path.clone() else { return };
        if self.is_busy || self.catalog.as_ref().and_then(|c| c.entries.get(index)).is_none_or(|e| e.is_directory) {
            return;
        }
        self.is_busy = true;
        self.busy_title = t("dialogs.preview.title");
        self.current_progress = None;
        self.send(WorkerCommand::LoadEntry { archive_path, index, password: self.password.clone() });
    }

    /// Double-click: built-in viewer for code/text/images, otherwise the associated program.
    fn open_entry(&mut self, idx: usize) {
        let path = self.catalog.as_ref().and_then(|c| c.entries.get(idx)).map(|e| e.path.clone()).unwrap_or_default();
        if viewer::prefers_builtin(&path) {
            self.open_viewer(idx);
        } else {
            self.open_external(idx);
        }
    }

    fn is_zip(&self) -> bool {
        self.catalog.as_ref().is_some_and(|c| c.format == ArchiveFormat::Zip)
    }

    /// Delete / add / replace ZIP entries, then re-read the archive in the same folder.
    fn modify_zip(&mut self, remove: Vec<usize>, add: Vec<(String, AddItem)>) {
        let Some(archive_path) = self.archive_path.clone() else { return };
        if self.is_busy || !self.is_zip() {
            return;
        }
        self.begin_task(
            TaskKind::Create { output: archive_path.clone(), sources: Vec::new(), format: ArchiveFormat::Zip, profile: self.config.default_compression_profile },
            t_format("msg.updating", &[("name", &stem_of(&archive_path))]),
        );
        self.send(WorkerCommand::ModifyZip { archive_path, remove, add, password: self.password.clone() });
    }

    /// Re-read the open archive, keeping password and current folder.
    fn reload_archive(&mut self) {
        let Some(path) = self.archive_path.clone() else { return };
        self.restore_nav = Some(self.table.nav_path.clone());
        self.is_busy = true;
        self.busy_title = t_format("msg.opening", &[("name", &stem_of(&path))]);
        self.current_progress = None;
        self.send(WorkerCommand::OpenArchive { path, password: self.password.clone() });
    }

    /// Add files/folders from disk into the current folder of the open ZIP.
    fn add_to_open_zip(&mut self, files: Vec<PathBuf>) {
        let prefix = if self.table.is_searching() { String::new() } else { self.table.nav_path.clone() };
        let add = files
            .into_iter()
            .filter_map(|p| {
                let name = p.file_name()?.to_string_lossy().to_string();
                Some((format!("{}{}", prefix, name), AddItem::Path(p)))
            })
            .collect();
        self.modify_zip(Vec::new(), add);
    }

    /// Extract many archives, each into "<name>\" next to it.
    fn extract_many(&mut self, files: Vec<PathBuf>) {
        for f in files {
            let out = self.default_extract_dir(&f);
            self.start_extract_on(f, out, None, false, self.config.default_compression_profile, OverwritePolicy::Overwrite);
        }
    }

    /// Extract one entry to a temp folder and open it with the associated program (WinRAR double-click).
    fn open_external(&mut self, idx: usize) {
        let Some(entry) = self.catalog.as_ref().and_then(|c| c.entries.get(idx)).cloned() else { return };
        if entry.is_directory || self.is_busy {
            return;
        }
        let dir = shell::new_temp_dir("open");
        let norm = entry.path.replace('\\', "/");
        let parent = crate::views::table::parent_of(&norm);
        let file_name = norm.rsplit('/').next().unwrap_or(&norm).to_string();
        self.pending_open = Some(dir.join(&file_name));
        let strip = (!parent.is_empty()).then_some(parent);
        self.start_extract(dir, Some((vec![idx], strip)), false, CompressionProfile::Balanced, OverwritePolicy::Overwrite);
    }

    pub fn start_hash(&mut self, file_path: PathBuf) {
        self.begin_task(TaskKind::Hash { file: file_path.clone() }, t("nav.hash_utility"));
        self.send(WorkerCommand::ComputeHashes { file_path });
    }

    pub fn start_compare(&mut self, archive_a: PathBuf, archive_b: PathBuf) {
        self.is_comparing = true;
        self.begin_task(
            TaskKind::Compare { archive_a: archive_a.clone(), archive_b: archive_b.clone() },
            t("nav.archive_compare"),
        );
        self.send(WorkerCommand::CompareArchives { archive_a, archive_b });
    }

    /// Extract the selected rows to a temp folder and hand them to Explorer as a real file drag.
    fn begin_drag(&mut self, frame: &eframe::Frame) {
        let Some(archive_path) = self.archive_path.clone() else { return };
        let rows: Vec<(String, Vec<usize>)> = self
            .table
            .selected_rows(&self.selected_indices)
            .into_iter()
            .map(|r| (r.name.clone(), r.indices.clone()))
            .collect();
        if rows.is_empty() {
            return;
        }
        let strip = (!self.table.is_searching() && !self.table.nav_path.is_empty()).then(|| self.table.nav_path.clone());
        let dir = shell::new_temp_dir("drag");
        let mut indices: Vec<usize> = rows.iter().flat_map(|r| r.1.iter().copied()).collect();
        indices.sort_unstable();
        indices.dedup();

        let options = ExtractionOptions {
            output_dir: dir.clone(),
            selected_indices: Some(indices),
            strip_prefix: strip,
            password: self.password.clone(),
            overwrite_policy: OverwritePolicy::Overwrite,
            ..Default::default()
        };
        // shortcut: synchronous extraction (UI pauses for big selections); a drag must start while the button is held.
        let result = OptiExtractor::extract(&archive_path, options, CancellationToken::new(), None::<GuiProgressListener>);
        match result {
            Ok(summary) if summary.failed_entries.is_empty() => {
                let paths: Vec<PathBuf> = rows.iter().map(|(name, _)| dir.join(name.trim_end_matches('/'))).filter(|p| p.exists()).collect();
                if let Err(e) = shell::start_drag(frame, paths) {
                    self.set_status(Err(e));
                }
            }
            Ok(summary) => {
                if summary.failed_entries.iter().any(|(_, e)| e.to_lowercase().contains("password")) {
                    self.password_dialog.is_open = true;
                    self.last_command = None;
                    self.set_status(Err(t("msg.password_then_retry")));
                } else if let Some((p, e)) = summary.failed_entries.first() {
                    self.set_status(Err(format!("{}: {}", p, e)));
                }
            }
            Err(e) => self.set_status(Err(e.to_string())),
        }
        // DoDragDrop swallowed the mouse-up; tell egui the button is released.
        self.inject_release = true;
    }

    fn set_status(&mut self, r: Result<String, String>) {
        match r {
            Ok(m) => {
                self.status_message = m;
                self.is_error_status = false;
            }
            Err(e) => {
                self.status_message = e;
                self.is_error_status = true;
            }
        }
    }

    // --------------------------------------------------------------- events

    fn handle_failure(&mut self, err: String) {
        self.last_failed = true;
        if err == PASSWORD_REQUIRED {
            self.password_dialog.input.clear();
            self.password_dialog.is_open = true;
            self.set_status(Err(if self.password.is_some() { t("msg.wrong_password") } else { t("msg.password_required") }));
        } else {
            if self.quick.is_some() {
                self.quick_errors.push(err.clone());
            }
            self.set_status(Err(err));
            self.pending_open = None;
            self.reveal_after = false;
        }
    }

    fn quick_done(&mut self, ctx: &egui::Context) {
        if self.quick.is_some() {
            self.quick_pending = self.quick_pending.saturating_sub(1);
            if self.quick_pending == 0 && self.quick_errors.is_empty() && !self.password_dialog.is_open {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn poll_worker_events(&mut self, ctx: &egui::Context) {
        while let Some(event) = self.worker.try_recv_event() {
            match event {
                WorkerEvent::ArchiveLoaded(res) => {
                    self.is_busy = !self.task_ids.is_empty();
                    match res {
                        Ok(cat) => {
                            self.set_status(Ok(t_format(
                                "msg.loaded",
                                &[
                                    ("count", &cat.entries.len().to_string()),
                                    ("size", &optirar_core::types::format_bytes(cat.total_uncompressed_bytes)),
                                ],
                            )));
                            self.catalog = Some(cat);
                            self.table.reset();
                            if let Some(nav) = self.restore_nav.take() {
                                self.table.nav_path = nav;
                            }
                            self.selected_indices.clear();
                            self.current_tab = NavTab::Explorer;
                            if let Some(p) = &self.archive_path {
                                ctx.send_viewport_cmd(ViewportCommand::Title(format!(
                                    "{} - OptiRAR",
                                    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                                )));
                            }
                        }
                        Err(e) => {
                            self.catalog = None;
                            if e != PASSWORD_REQUIRED {
                                self.archive_path = None;
                                self.handle_failure(t_format("msg.open_failed", &[("error", &e)]));
                            } else {
                                self.handle_failure(e);
                            }
                        }
                    }
                }
                WorkerEvent::Progress(snap) => {
                    if let Some((id, _)) = self.task_ids.front() {
                        self.queue.update_state(
                            *id,
                            TaskState::Running {
                                progress: snap.percent() as f32,
                                current_file: snap.current_file.clone(),
                                throughput_mb_s: snap.throughput_bytes_per_sec / (1024.0 * 1024.0),
                            },
                        );
                    }
                    self.current_progress = Some(snap);
                }
                WorkerEvent::ExtractCompleted(res) => {
                    match res {
                        Ok((msg, out_dir)) => {
                            self.finish_task(Ok(&msg));
                            if let Some(file) = self.pending_open.take() {
                                shell::open_with_system(&file);
                            } else if std::mem::take(&mut self.reveal_after) || (self.config.open_folder_after_extract && self.quick.is_none()) {
                                shell::open_with_system(&out_dir);
                            }
                            self.set_status(Ok(msg));
                            self.quick_done(ctx);
                        }
                        Err(e) => {
                            self.finish_task(Err(&e));
                            self.handle_failure(e);
                        }
                    }
                }
                WorkerEvent::TestCompleted(res) => match res {
                    Ok(msg) => {
                        self.finish_task(Ok(&msg));
                        self.set_status(Ok(msg.clone()));
                        if self.quick.is_some() {
                            // Keep the window open so the user can read the result.
                            self.quick_errors.push(format!("✓ {}", msg));
                        }
                    }
                    Err(e) => {
                        self.finish_task(Err(&e));
                        self.handle_failure(e);
                    }
                },
                WorkerEvent::CreateCompleted(res) => match res {
                    Ok(msg) => {
                        self.finish_task(Ok(&msg));
                        self.set_status(Ok(msg));
                        self.quick_done(ctx);
                    }
                    Err(e) => {
                        self.finish_task(Err(&e));
                        self.handle_failure(e);
                    }
                },
                WorkerEvent::EntryLoaded(res) => {
                    self.is_busy = !self.task_ids.is_empty();
                    match res {
                        Ok((index, bytes, truncated)) => {
                            if let Some(entry) = self.catalog.as_ref().and_then(|c| c.entries.get(index)) {
                                let writable = self.is_zip();
                                self.viewer = Some(ViewerState::new(index, entry.path.clone(), entry.uncompressed_size, truncated, bytes, writable));
                            }
                        }
                        Err(e) => self.handle_failure(e),
                    }
                }
                WorkerEvent::ModifyCompleted(res) => match res {
                    Ok(msg) => {
                        self.finish_task(Ok(&msg));
                        if let Some(v) = self.viewer.as_mut() {
                            v.mark_saved();
                        }
                        self.reload_archive();
                        self.set_status(Ok(msg));
                    }
                    Err(e) => {
                        self.finish_task(Err(&e));
                        self.set_status(Err(e));
                    }
                },
                WorkerEvent::HashesComputed(res) => match res {
                    Ok(hashes) => {
                        self.finish_task(Ok(""));
                        self.file_hashes = Some(hashes);
                    }
                    Err(e) => {
                        self.finish_task(Err(&e));
                        self.set_status(Err(e));
                    }
                },
                WorkerEvent::CompareCompleted(res) => {
                    self.is_comparing = false;
                    match res {
                        Ok(diff) => {
                            self.finish_task(Ok(""));
                            self.archive_diff = Some(diff);
                        }
                        Err(e) => {
                            self.finish_task(Err(&e));
                            self.set_status(Err(e));
                        }
                    }
                }
            }
        }
    }

    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        // Ignore our own drag-out landing back on this window (temp files of this session).
        let session = shell::session_dir();
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).filter(|p| !p.starts_with(&session)).collect());
        if dropped.is_empty() || self.is_busy {
            return;
        }
        if self.current_tab == NavTab::Explorer && self.is_zip() {
            self.pending_drop = Some(dropped);
        } else if dropped.len() == 1 && is_archive(&dropped[0]) {
            self.open_archive(dropped[0].clone());
        } else {
            // Dropping ordinary files/folders = "Add to archive" (WinRAR behaviour).
            self.create_dialog.open_with(dropped);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) && !self.is_busy {
            if let Some(path) = pick_archive() {
                self.open_archive(path);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.is_busy {
                self.worker.cancel();
            } else if self.viewer.as_ref().is_some_and(|v| v.open) {
                self.viewer = None;
            } else if !self.search_query.is_empty() {
                self.search_query.clear();
            } else {
                self.selected_indices.clear();
            }
        }
    }

    // ------------------------------------------------------------------- UI

    fn quick_ui(&mut self, ctx: &egui::Context) {
        CentralPanel::default().show(ctx, |ui| {
            if self.is_busy {
                return;
            }
            ui.add_space(8.0);
            let ok = self.quick_errors.iter().all(|e| e.starts_with('✓'));
            let (icon, color) = if ok { ("✓", theme::SUCCESS) } else { ("⚠", theme::ERROR) };
            ui.label(RichText::new(format!("{} OptiRAR", icon)).size(18.0).strong().color(color));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                for e in &self.quick_errors {
                    ui.label(e.trim_start_matches("✓ "));
                }
                if self.quick_errors.is_empty() && !self.status_message.is_empty() {
                    ui.label(&self.status_message);
                }
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(t("dialogs.properties.close")).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                if let Some(p) = self.archive_path.clone() {
                    if ui.button(t("quick.open_in_app")).clicked() {
                        self.quick = None;
                        self.quick_errors.clear();
                        ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(1100.0, 700.0)));
                        self.open_archive(p);
                    }
                }
            });
        });
    }

    fn explorer_ui(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let Some(cat) = self.catalog.as_ref() else {
            let action = WelcomeView::show(ui, &self.config.recent_archives);
            self.handle_welcome(action);
            return;
        };

        let has_selection = !self.selected_indices.is_empty();
        let action = ToolbarView::show(ui, true, has_selection, self.is_busy);
        ui.add_space(2.0);

        let stem = self.archive_path.as_deref().map(stem_of).unwrap_or_default();
        let writable = cat.format == ArchiveFormat::Zip;
        let tbl = TableView::show(ui, cat, &mut self.table, &self.search_query, &mut self.selected_indices, &stem, writable);

        if action.open_clicked {
            if let Some(path) = pick_archive() {
                self.open_archive(path);
            }
        }
        if action.create_clicked {
            if self.is_zip() {
                if let Some(files) = rfd::FileDialog::new().set_title(t("dialogs.drop.title")).pick_files() {
                    self.add_to_open_zip(files);
                }
            } else {
                self.create_dialog.open_with(Vec::new());
            }
        }
        if action.extract_clicked || tbl.extract_selected_dialog || tbl.extract_all_dialog {
            let selected = (has_selection && !tbl.extract_all_dialog) || tbl.extract_selected_dialog;
            self.extract_dialog.extract_selected_only = selected;
            self.extract_dialog.is_open = true;
        }
        if action.test_clicked {
            self.start_test();
        }
        if action.info_clicked || tbl.show_properties {
            self.show_properties = true;
        }
        if action.view_clicked {
            let first = self.table.selected_rows(&self.selected_indices).into_iter().find(|r| !r.is_dir).and_then(|r| r.indices.first().copied());
            if let Some(idx) = first {
                self.open_viewer(idx);
            }
        }

        if let Some(target) = tbl.navigate_to {
            self.table.nav_path = target;
            self.search_query.clear();
        }
        if let Some(idx) = tbl.view_entry {
            self.open_viewer(idx);
        }
        if let Some(idx) = tbl.open_external {
            self.open_external(idx);
        }
        if let Some(idx) = tbl.open_entry {
            self.open_entry(idx);
        }
        if tbl.delete_selected && !self.selected_indices.is_empty() {
            let mut v: Vec<usize> = self.selected_indices.iter().copied().collect();
            v.sort_unstable();
            if self.config.confirm_delete {
                self.confirm_delete = Some(v);
            } else {
                self.modify_zip(v, Vec::new());
            }
        }
        if let Some(archive) = self.archive_path.clone() {
            if tbl.extract_selected_here {
                self.extract_selected_to(parent_dir(&archive), false);
            }
            if tbl.extract_selected_subfolder {
                self.extract_selected_to(parent_dir(&archive).join(stem_of(&archive)), false);
            }
        }
        if tbl.start_drag && !self.is_busy {
            self.begin_drag(frame);
        }
    }

    fn handle_welcome(&mut self, action: crate::views::welcome::WelcomeAction) {
        if action.open_clicked {
            if let Some(path) = pick_archive() {
                self.open_archive(path);
            }
        }
        if action.create_clicked {
            self.create_dialog.open_with(Vec::new());
        }
        if action.hash_clicked {
            self.current_tab = NavTab::Tools;
            self.tools_subtab = 0;
        }
        if action.extract_many_clicked {
            if let Some(files) = rfd::FileDialog::new().add_filter(t("dialogs.open_filter"), ARCHIVE_EXTS).pick_files() {
                self.extract_many(files);
            }
        }
        if let Some(p) = action.open_recent {
            self.open_archive(p);
        }
        if let Some(p) = action.remove_recent {
            self.config.remove_recent(&p);
            let _ = self.config.save();
        }
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let action = SettingsView::show(ui, &mut self.config, self.shell_status_msg.as_ref());
        if let Some(th) = action.theme_changed {
            self.dark_mode = th != ThemeMode::Light;
            configure_visuals(ctx, self.dark_mode);
        }
        if action.language_changed.is_some() {
            self.table.reset();
        }
        if action.register_shell_clicked || action.unregister_shell_clicked {
            let register = action.register_shell_clicked;
            self.shell_status_msg = Some(match shell::run_shell_script(register, &self.config.language.code()[..2], true) {
                Ok(()) => {
                    self.config.shell_integration_enabled = register;
                    let _ = self.config.save();
                    (true, t(if register { "msg.shell_registered" } else { "msg.shell_removed" }))
                }
                Err(e) => (false, t_format("msg.shell_failed", &[("error", &e)])),
            });
        }
        if action.save_config_clicked {
            let _ = self.config.save();
        }
        if action.codepage_changed {
            optirar_formats::zip_format::set_name_codepage(self.config.zip_name_codepage);
            if self.is_zip() {
                self.reload_archive();
            }
        }
    }

    fn tools_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (i, key) in ["nav.hash_utility", "nav.archive_compare", "nav.duplicate_finder"].iter().enumerate() {
                if ui.selectable_label(self.tools_subtab == i, t(key)).clicked() {
                    self.tools_subtab = i;
                }
            }
        });
        ui.separator();

        match self.tools_subtab {
            0 => {
                let mut picked_file = None;
                HashView::show(ui, &mut self.hash_state, self.file_hashes.as_ref(), |file| picked_file = Some(file));
                if let Some(file) = picked_file {
                    self.file_hashes = None;
                    self.start_hash(file);
                }
            }
            1 => {
                let mut pair = None;
                CompareView::show(ui, &mut self.compare_state, self.archive_diff.as_ref(), self.is_comparing, |a, b| pair = Some((a, b)));
                if let Some((a, b)) = pair {
                    self.start_compare(a, b);
                }
            }
            _ => DuplicatesView::show(ui, self.catalog.as_ref()),
        }
    }

    fn about_ui(ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);
            ui.label(RichText::new("📦 OptiRAR").size(26.0).strong().color(theme::accent_text(ui)));
            ui.label(RichText::new(format!("{} {}", t("about.version"), env!("CARGO_PKG_VERSION"))).color(theme::weak_text(ui)));
            ui.add_space(14.0);
            theme::card(ui).show(ui, |ui| {
                ui.set_max_width(540.0);
                for (h, b) in [("about.philosophy", "app.tagline"), ("about.stack", "about.stack_desc"), ("about.license", "about.license_desc")] {
                    ui.label(RichText::new(t(h)).strong());
                    ui.label(t(b));
                    ui.add_space(8.0);
                }
            });
        });
    }
}

impl eframe::App for OptiRarApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if std::mem::take(&mut self.inject_release) {
            let pos = ctx.input(|i| i.pointer.latest_pos()).unwrap_or_default();
            raw_input.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            });
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let was_busy = self.is_busy;
        self.poll_worker_events(ctx);
        self.window_size = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
        self.update_taskbar(ctx, frame, was_busy);
        self.fit_to_screen(ctx);

        if self.quick.is_some() {
            if self.is_busy {
                let worker = &self.worker;
                ProgressModalView::show(ctx, &self.busy_title, self.current_progress.as_ref(), || worker.cancel());
            }
            self.quick_ui(ctx);
            if let Some(pw) = DialogsView::show_password_dialog(ctx, &mut self.password_dialog) {
                self.retry_with_password(pw);
            }
            return;
        }

        self.handle_drag_and_drop(ctx);
        self.handle_shortcuts(ctx);

        let is_solid = self.catalog.as_ref().map(|c| c.is_solid).unwrap_or(false);

        TopBottomPanel::top("optirar_top_titlebar").exact_height(40.0).show(ctx, |ui| {
            let action = TitleBarView::show(ui, self.archive_path.as_deref(), is_solid, &mut self.search_query, self.dark_mode);
            if action.toggle_theme {
                self.dark_mode = !self.dark_mode;
                self.config.theme = if self.dark_mode { ThemeMode::Dark } else { ThemeMode::Light };
                configure_visuals(ctx, self.dark_mode);
                let _ = self.config.save();
            }
            if action.toggle_language {
                let next = match get_i18n().language() {
                    Language::EnUs => Language::ViVn,
                    Language::ViVn => Language::EnUs,
                };
                get_i18n().set_language(next);
                self.config.language = next;
                self.table.reset();
                let _ = self.config.save();
            }
            if action.open_settings {
                self.current_tab = NavTab::Settings;
            }
            if action.open_about {
                self.current_tab = NavTab::About;
            }
        });

        TopBottomPanel::bottom("optirar_bottom_status").exact_height(26.0).show(ctx, |ui| {
            StatusBarView::show(ui, self.catalog.as_ref(), &self.selected_indices, &self.status_message, self.is_error_status);
        });

        SidePanel::left("optirar_left_sidebar").resizable(false).show(ctx, |ui| {
            let has_active = self.queue.has_active_tasks();
            SidebarView::show(ui, &mut self.current_tab, &mut self.sidebar_collapsed, has_active);
        });

        CentralPanel::default().show(ctx, |ui| match self.current_tab {
            NavTab::Home => {
                let action = WelcomeView::show(ui, &self.config.recent_archives);
                self.handle_welcome(action);
            }
            NavTab::Explorer => self.explorer_ui(ui, frame),
            NavTab::Queue => QueueView::show(ui, &self.queue),
            NavTab::Tools => self.tools_ui(ui),
            NavTab::Settings => self.settings_ui(ui, ctx),
            NavTab::About => Self::about_ui(ui),
        });

        if self.is_busy {
            let worker = &self.worker;
            ProgressModalView::show(ctx, &self.busy_title, self.current_progress.as_ref(), || worker.cancel());
        }

        self.viewer_ui(ctx);
        self.extra_dialogs(ctx);
        DialogsView::show_properties_dialog(ctx, &mut self.show_properties, self.catalog.as_ref(), self.archive_path.as_deref());

        let mut create_payload = None;
        DialogsView::show_create_dialog(ctx, &mut self.create_dialog, |dest, srcs, fmt, lvl, pw| {
            create_payload = Some((dest, srcs, fmt, lvl, pw));
        });
        if let Some((dest, srcs, fmt, lvl, pw)) = create_payload {
            self.start_create(dest, srcs, fmt, lvl, pw);
        }

        let mut extract_payload = None;
        let sel_count = self.table.selected_rows(&self.selected_indices).len();
        let mut smart = self.config.smart_extract;
        let mut open_after = self.config.open_folder_after_extract;
        DialogsView::show_extract_dialog(ctx, &mut self.extract_dialog, sel_count, &mut smart, &mut open_after, |dest, prof, ovw| {
            extract_payload = Some((dest, prof, ovw));
        });
        if smart != self.config.smart_extract || open_after != self.config.open_folder_after_extract {
            self.config.smart_extract = smart;
            self.config.open_folder_after_extract = open_after;
            let _ = self.config.save();
        }
        if let Some((dest, prof, ovw)) = extract_payload {
            let (sel, strip) = self.selection_for_extract(self.extract_dialog.extract_selected_only);
            let smart = self.config.smart_extract && sel.is_none();
            self.start_extract(dest, sel.map(|v| (v, strip)), smart, prof, ovw);
        }

        if let Some(pw) = DialogsView::show_password_dialog(ctx, &mut self.password_dialog) {
            self.retry_with_password(pw);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.quick.is_none() {
            if let Some(size) = self.window_size {
                self.config.window_width = size.x;
                self.config.window_height = size.y;
                let _ = self.config.save();
            }
        }
        shell::cleanup_session();
    }
}

impl OptiRarApp {
    fn update_taskbar(&mut self, ctx: &egui::Context, frame: &eframe::Frame, was_busy: bool) {
        let state = if self.is_busy {
            match &self.current_progress {
                Some(p) => TaskbarState::Progress((p.percent() / 100.0) as f32),
                None => TaskbarState::Indeterminate,
            }
        } else if self.last_failed && was_busy {
            TaskbarState::Error
        } else if was_busy {
            TaskbarState::None
        } else {
            return;
        };
        self.taskbar.set(frame, state);
        if was_busy && !self.is_busy {
            let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
            if !focused {
                self.taskbar.flash(frame);
            }
            self.last_failed = false;
        }
        if self.is_busy && !was_busy {
            self.last_failed = false;
        }
    }

    fn viewer_ui(&mut self, ctx: &egui::Context) {
        let Some(v) = self.viewer.as_mut() else { return };
        let action = v.show(ctx);
        let (index, path, open) = (v.index, v.path.clone(), v.open);
        if let Some(bytes) = action.save_to_archive {
            self.modify_zip(vec![index], vec![(path.clone(), AddItem::Bytes(bytes))]);
        }
        if let Some(bytes) = action.save_as {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            if let Some(dest) = rfd::FileDialog::new().set_file_name(&name).save_file() {
                let r = std::fs::write(&dest, bytes).map(|_| t_format("msg.saved_to", &[("path", &dest.display().to_string())]));
                self.set_status(r.map_err(|e| e.to_string()));
            }
        }
        if action.open_external {
            self.open_external(index);
        }
        if !open {
            self.viewer = None;
        }
    }

    fn extra_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(files) = self.pending_drop.clone() {
            let name = self.archive_path.as_deref().map(stem_of).unwrap_or_default();
            match drop_choice_dialog(ctx, files.len(), &name, &self.table.nav_path) {
                Some(DropChoice::AddToArchive) => {
                    self.pending_drop = None;
                    self.add_to_open_zip(files);
                }
                Some(DropChoice::NewArchive) => {
                    self.pending_drop = None;
                    self.create_dialog.open_with(files);
                }
                Some(DropChoice::Cancel) => self.pending_drop = None,
                None => {}
            }
        }
        if let Some(indices) = self.confirm_delete.clone() {
            let n = indices.len().to_string();
            match confirm_dialog(ctx, &t("dialogs.delete.title"), &t_format("dialogs.delete.prompt", &[("count", &n)]), &t("ctx.delete")) {
                Some(true) => {
                    self.confirm_delete = None;
                    self.selected_indices.clear();
                    self.modify_zip(indices, Vec::new());
                }
                Some(false) => self.confirm_delete = None,
                None => {}
            }
        }
    }

    /// A remembered window size can be taller than the screen (e.g. 1080p at 150%): shrink and re-center once.
    fn fit_to_screen(&mut self, ctx: &egui::Context) {
        if self.fitted_to_screen {
            return;
        }
        let (monitor, outer, inner) = ctx.input(|i| (i.viewport().monitor_size, i.viewport().outer_rect, i.viewport().inner_rect));
        let (Some(monitor), Some(outer), Some(inner)) = (monitor, outer, inner) else { return };
        self.fitted_to_screen = true;
        let chrome = outer.size() - inner.size();
        let max = monitor - chrome - egui::vec2(16.0, 64.0); // leave room for the taskbar
        let size = inner.size();
        if size.x > max.x || size.y > max.y {
            let new = size.min(max).max(egui::vec2(640.0, 400.0));
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(new));
            let pos = ((monitor - new - chrome) / 2.0).max(egui::Vec2::ZERO);
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(egui::pos2(pos.x, (pos.y - 16.0).max(0.0))));
        }
    }

    fn retry_with_password(&mut self, pw: String) {
        self.password = Some(pw.clone());
        if let Some(cmd) = self.last_command.take() {
            let cmd = with_password(&cmd, &pw);
            let title = self.busy_title.clone();
            match &cmd {
                WorkerCommand::Extract { archive_path, output_dir, .. } => {
                    self.begin_task(
                        TaskKind::Extract { archive: archive_path.clone(), destination: output_dir.clone(), smart: false },
                        title,
                    );
                }
                WorkerCommand::TestArchive { archive_path, .. } => {
                    self.begin_task(TaskKind::Test { archive: archive_path.clone() }, title);
                }
                _ => {
                    self.is_busy = true;
                    self.current_progress = None;
                }
            }
            self.send(cmd);
        }
    }
}
