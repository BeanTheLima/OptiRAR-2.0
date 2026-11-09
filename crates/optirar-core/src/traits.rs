use crate::error::Result;
use crate::types::{ArchiveEntry, ArchiveFormat, ArchiveMetadata};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A thread-safe cancellation token.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}

/// Snapshot of extraction or compression progress.
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressSnapshot {
    pub files_completed: usize,
    pub total_files: usize,
    pub bytes_completed: u64,
    pub total_bytes: u64,
    pub current_file: String,
    pub throughput_bytes_per_sec: f64,
    pub elapsed_seconds: f64,
    pub eta_seconds: Option<f64>,
}

impl ProgressSnapshot {
    /// Percentage completed (0.0 to 100.0).
    pub fn percent(&self) -> f64 {
        if self.total_bytes == 0 {
            if self.total_files == 0 {
                0.0
            } else {
                (self.files_completed as f64 / self.total_files as f64) * 100.0
            }
        } else {
            (self.bytes_completed as f64 / self.total_bytes as f64) * 100.0
        }
    }

    /// Human-friendly throughput string (e.g. "245.8 MB/s").
    pub fn display_throughput(&self) -> String {
        let mb_s = self.throughput_bytes_per_sec / (1024.0 * 1024.0);
        format!("{:.1} MB/s", mb_s)
    }

    /// Human-friendly estimated time remaining string.
    pub fn display_eta(&self) -> String {
        match self.eta_seconds {
            Some(secs) if secs < 60.0 => format!("{:.0}s remaining", secs),
            Some(secs) if secs < 3600.0 => {
                let mins = (secs / 60.0).floor();
                let rem = (secs % 60.0).round();
                format!("{}m {}s remaining", mins, rem)
            }
            Some(secs) => {
                let hrs = (secs / 3600.0).floor();
                let mins = ((secs % 3600.0) / 60.0).floor();
                format!("{}h {}m remaining", hrs, mins)
            }
            None => "Calculating...".to_string(),
        }
    }
}

/// Callback trait for progress events during extraction or compression.
pub trait ProgressListener: Send + Sync {
    fn on_progress(&self, snapshot: &ProgressSnapshot);
}

/// Blanket implementation for closures.
impl<F> ProgressListener for F
where
    F: Fn(&ProgressSnapshot) + Send + Sync,
{
    fn on_progress(&self, snapshot: &ProgressSnapshot) {
        (self)(snapshot);
    }
}

/// Unified trait implemented by format-specific archive readers.
pub trait ArchiveReader: Send + Sync {
    /// Inspect and return the list of entries inside the archive.
    fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>>;

    /// Extract a single entry by its index, writing raw uncompressed bytes to `writer`.
    fn extract_entry(&mut self, entry_index: usize, writer: &mut dyn Write) -> Result<()>;

    /// Returns true if this archive uses solid compression blocks.
    fn is_solid(&self) -> bool;

    /// Return the detected archive format.
    fn format(&self) -> ArchiveFormat;

    /// Optional extended metadata.
    fn metadata(&self) -> Option<ArchiveMetadata> {
        None
    }

    /// Cheap independent handle sharing the already-parsed index (for parallel extraction).
    fn try_clone(&self) -> Option<Box<dyn ArchiveReader>> {
        None
    }
}

/// Unified trait implemented by format-specific archive writers.
pub trait ArchiveWriter: Send {
    /// Add an existing file on disk into the archive at the given relative path.
    fn add_file(&mut self, archive_path: &str, source_path: &Path) -> Result<()>;

    /// Add raw bytes from a reader into the archive at the given relative path.
    fn add_stream(&mut self, archive_path: &str, reader: &mut dyn Read, size: u64) -> Result<()>;

    /// Finalize and flush the archive container.
    fn finish(self: Box<Self>) -> Result<()>;
}
