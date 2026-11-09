use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use optirar_core::i18n::t_format;
use optirar_core::t;
use optirar_core::traits::{CancellationToken, ProgressListener, ProgressSnapshot};
use optirar_core::types::{
    format_bytes, ArchiveFormat, CompressionLevel, CompressionProfile, ExtractionOptions,
    OverwritePolicy,
};
use optirar_engine::compare::{ArchiveComparator, ArchiveDiffResult};
use optirar_engine::compressor::OptiCompressor;
use optirar_engine::extractor::{ExtractionSummary, OptiExtractor};
use optirar_engine::preview::ArchivePreviewer;
use optirar_engine::smart_extract::compute_smart_destination;
use optirar_formats::factory::open_reader;
use optirar_formats::{modify_zip, ZipAddSource};
use optirar_index::catalog::ArchiveCatalog;
use optirar_integrity::{FileHashes, MultiHasher};
use std::path::{Path, PathBuf};
use std::thread;

/// Error string returned when an operation failed because a password is missing or wrong.
pub const PASSWORD_REQUIRED: &str = "__OPTIRAR_PASSWORD_REQUIRED__";

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    OpenArchive {
        path: PathBuf,
        password: Option<String>,
    },
    Extract {
        archive_path: PathBuf,
        output_dir: PathBuf,
        selected_indices: Option<Vec<usize>>,
        strip_prefix: Option<String>,
        password: Option<String>,
        profile: CompressionProfile,
        overwrite: OverwritePolicy,
        smart: bool,
    },
    TestArchive {
        archive_path: PathBuf,
        password: Option<String>,
    },
    CreateArchive {
        destination: PathBuf,
        sources: Vec<PathBuf>,
        format: ArchiveFormat,
        level: CompressionLevel,
        password: Option<String>,
    },
    /// Read one entry fully (up to a cap) for the built-in viewer.
    LoadEntry {
        archive_path: PathBuf,
        index: usize,
        password: Option<String>,
    },
    /// Delete / add / replace entries of a ZIP in place.
    ModifyZip {
        archive_path: PathBuf,
        remove: Vec<usize>,
        add: Vec<(String, AddItem)>,
        password: Option<String>,
    },
    ComputeHashes {
        file_path: PathBuf,
    },
    CompareArchives {
        archive_a: PathBuf,
        archive_b: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub enum AddItem {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

/// Viewer reads at most this much of one entry.
pub const VIEWER_MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone)]
pub enum WorkerEvent {
    ArchiveLoaded(Result<ArchiveCatalog, String>),
    Progress(ProgressSnapshot),
    /// Ok((message, output directory)).
    ExtractCompleted(Result<(String, PathBuf), String>),
    TestCompleted(Result<String, String>),
    CreateCompleted(Result<String, String>),
    /// Ok((index, bytes, truncated))
    EntryLoaded(Result<(usize, Vec<u8>, bool), String>),
    ModifyCompleted(Result<String, String>),
    HashesComputed(Result<FileHashes, String>),
    CompareCompleted(Result<ArchiveDiffResult, String>),
}

pub struct GuiProgressListener {
    sender: Sender<WorkerEvent>,
    ctx: egui::Context,
}

impl GuiProgressListener {
    pub fn new(sender: Sender<WorkerEvent>, ctx: egui::Context) -> Self {
        Self { sender, ctx }
    }
}

impl ProgressListener for GuiProgressListener {
    fn on_progress(&self, snapshot: &ProgressSnapshot) {
        let _ = self.sender.send(WorkerEvent::Progress(snapshot.clone()));
        self.ctx.request_repaint();
    }
}

pub struct WorkerHandle {
    cmd_sender: Sender<WorkerCommand>,
    event_receiver: Receiver<WorkerEvent>,
    active_cancel: CancellationToken,
}

fn is_password_error(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("password") || m.contains("encrypted") || m.contains("decrypt")
}

fn map_err(e: impl ToString) -> String {
    let s = e.to_string();
    if is_password_error(&s) {
        PASSWORD_REQUIRED.to_string()
    } else {
        s
    }
}

/// Turn per-file failures into an error (password problems get the marker).
fn check_failures(summary: &ExtractionSummary) -> Result<(), String> {
    if let Some((path, err)) = summary.failed_entries.first() {
        if summary.failed_entries.iter().any(|(_, e)| is_password_error(e)) {
            return Err(PASSWORD_REQUIRED.to_string());
        }
        return Err(t_format(
            "msg.partial_failure",
            &[
                ("failed", &summary.failed_entries.len().to_string()),
                ("path", path),
                ("error", err),
            ],
        ));
    }
    Ok(())
}

impl WorkerHandle {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<WorkerCommand>();
        let (evt_tx, evt_rx) = crossbeam_channel::unbounded::<WorkerEvent>();
        let active_cancel = CancellationToken::new();
        let cancel = active_cancel.clone();

        thread::Builder::new()
            .name("optirar-gui-worker".to_string())
            .spawn(move || {
                while let Ok(cmd) = cmd_rx.recv() {
                    cancel.reset();
                    let event = match cmd {
                        WorkerCommand::OpenArchive { path, password } => {
                            WorkerEvent::ArchiveLoaded(Self::handle_open(&path, password.as_deref()))
                        }
                        WorkerCommand::Extract {
                            archive_path,
                            output_dir,
                            selected_indices,
                            strip_prefix,
                            password,
                            profile,
                            overwrite,
                            smart,
                        } => WorkerEvent::ExtractCompleted(Self::handle_extract(
                            &archive_path,
                            output_dir,
                            selected_indices,
                            strip_prefix,
                            password,
                            profile,
                            overwrite,
                            smart,
                            GuiProgressListener::new(evt_tx.clone(), ctx.clone()),
                            cancel.clone(),
                        )),
                        WorkerCommand::TestArchive { archive_path, password } => WorkerEvent::TestCompleted(
                            Self::handle_test(&archive_path, password.as_deref(), cancel.clone()),
                        ),
                        WorkerCommand::CreateArchive { destination, sources, format, level, password } => {
                            WorkerEvent::CreateCompleted(Self::handle_create(
                                &destination,
                                &sources,
                                format,
                                level,
                                password.as_deref(),
                                GuiProgressListener::new(evt_tx.clone(), ctx.clone()),
                                cancel.clone(),
                            ))
                        }
                        WorkerCommand::LoadEntry { archive_path, index, password } => WorkerEvent::EntryLoaded(
                            ArchivePreviewer::read_entry(&archive_path, index, password.as_deref(), VIEWER_MAX_BYTES)
                                .map(|(bytes, cut)| (index, bytes, cut))
                                .map_err(map_err),
                        ),
                        WorkerCommand::ModifyZip { archive_path, remove, add, password } => {
                            let remove = remove.into_iter().collect();
                            let add: Vec<(String, ZipAddSource)> = add
                                .into_iter()
                                .map(|(n, a)| (n, match a { AddItem::Path(p) => ZipAddSource::Path(p), AddItem::Bytes(b) => ZipAddSource::Bytes(b) }))
                                .collect();
                            WorkerEvent::ModifyCompleted(
                                modify_zip(&archive_path, &remove, &add, CompressionLevel::Normal, password.as_deref())
                                    .map(|_| t("msg.archive_updated"))
                                    .map_err(|e| e.to_string()),
                            )
                        }
                        WorkerCommand::ComputeHashes { file_path } => WorkerEvent::HashesComputed(
                            MultiHasher::compute_all(&file_path, Some(&cancel)).map_err(|e| e.to_string()),
                        ),
                        WorkerCommand::CompareArchives { archive_a, archive_b } => {
                            WorkerEvent::CompareCompleted(Self::handle_compare(&archive_a, &archive_b))
                        }
                    };
                    let _ = evt_tx.send(event);
                    ctx.request_repaint();
                }
            })
            .expect("Failed to spawn OptiRAR GUI worker thread");

        Self {
            cmd_sender: cmd_tx,
            event_receiver: evt_rx,
            active_cancel,
        }
    }

    pub fn send_command(&self, cmd: WorkerCommand) {
        let _ = self.cmd_sender.send(cmd);
    }

    pub fn try_recv_event(&self) -> Option<WorkerEvent> {
        self.event_receiver.try_recv().ok()
    }

    /// Cancel the running operation (the worker resets the token before the next command).
    pub fn cancel(&self) {
        self.active_cancel.cancel();
    }

    fn handle_open(path: &Path, password: Option<&str>) -> Result<ArchiveCatalog, String> {
        let mut reader = open_reader(path, password).map_err(map_err)?;
        let format = reader.format();
        let is_solid = reader.is_solid();
        let entries = reader.list_entries().map_err(map_err)?;
        Ok(ArchiveCatalog::new(format, entries, is_solid))
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_extract(
        archive_path: &Path,
        output_dir: PathBuf,
        selected_indices: Option<Vec<usize>>,
        strip_prefix: Option<String>,
        password: Option<String>,
        profile: CompressionProfile,
        overwrite: OverwritePolicy,
        smart: bool,
        listener: GuiProgressListener,
        cancel: CancellationToken,
    ) -> Result<(String, PathBuf), String> {
        let output_dir = if smart && selected_indices.is_none() {
            let mut reader = open_reader(archive_path, password.as_deref()).map_err(map_err)?;
            let entries = reader.list_entries().map_err(map_err)?;
            compute_smart_destination(archive_path, &output_dir, &entries)
        } else {
            output_dir
        };

        let options = ExtractionOptions {
            output_dir: output_dir.clone(),
            overwrite_policy: overwrite,
            profile,
            selected_indices,
            preserve_attributes: true,
            preserve_timestamps: true,
            password,
            keep_broken: false,
            strip_prefix,
        };

        let summary = OptiExtractor::extract(archive_path, options, cancel, Some(listener)).map_err(map_err)?;
        check_failures(&summary)?;

        Ok((
            t_format(
                "msg.extract_ok",
                &[
                    ("files", &summary.files_extracted.to_string()),
                    ("size", &format_bytes(summary.bytes_extracted)),
                    ("secs", &format!("{:.2}", summary.elapsed.as_secs_f64())),
                    ("speed", &format!("{:.1}", summary.average_throughput_mb_s)),
                ],
            ),
            output_dir,
        ))
    }

    fn handle_test(archive_path: &Path, password: Option<&str>, cancel: CancellationToken) -> Result<String, String> {
        let summary = OptiExtractor::test_archive(archive_path, password, cancel).map_err(map_err)?;
        check_failures(&summary)?;
        Ok(t_format(
            "msg.test_ok",
            &[
                ("files", &summary.files_extracted.to_string()),
                ("size", &format_bytes(summary.bytes_extracted)),
            ],
        ))
    }

    fn handle_create(
        destination: &Path,
        sources: &[PathBuf],
        format: ArchiveFormat,
        level: CompressionLevel,
        password: Option<&str>,
        listener: GuiProgressListener,
        cancel: CancellationToken,
    ) -> Result<String, String> {
        let summary = OptiCompressor::create_archive_with_password(destination, sources, format, level, password, cancel, Some(listener))
            .map_err(|e| e.to_string())?;

        Ok(t_format(
            "msg.create_ok",
            &[
                ("files", &summary.files_added.to_string()),
                ("size", &format_bytes(summary.bytes_compressed)),
                ("secs", &format!("{:.2}", summary.elapsed.as_secs_f64())),
            ],
        ))
    }

    fn handle_compare(archive_a: &Path, archive_b: &Path) -> Result<ArchiveDiffResult, String> {
        let entries_a = open_reader(archive_a, None).and_then(|mut r| r.list_entries()).map_err(|e| e.to_string())?;
        let entries_b = open_reader(archive_b, None).and_then(|mut r| r.list_entries()).map_err(|e| e.to_string())?;
        Ok(ArchiveComparator::compare(&entries_a, &entries_b))
    }
}
