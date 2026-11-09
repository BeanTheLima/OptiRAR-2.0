use optirar_core::traits::{ProgressListener, ProgressSnapshot};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// High-precision, lock-free progress tracker with real-time throughput calculations.
#[derive(Clone)]
pub struct ProgressTracker {
    start_time: Instant,
    total_files: usize,
    total_bytes: u64,
    files_completed: Arc<AtomicUsize>,
    bytes_completed: Arc<AtomicU64>,
    listeners: Vec<Arc<dyn ProgressListener>>,
}

impl std::fmt::Debug for ProgressTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressTracker")
            .field("total_files", &self.total_files)
            .field("total_bytes", &self.total_bytes)
            .field("files_completed", &self.files_completed)
            .field("bytes_completed", &self.bytes_completed)
            .finish()
    }
}

impl ProgressTracker {
    pub fn new(total_files: usize, total_bytes: u64) -> Self {
        Self {
            start_time: Instant::now(),
            total_files,
            total_bytes,
            files_completed: Arc::new(AtomicUsize::new(0)),
            bytes_completed: Arc::new(AtomicU64::new(0)),
            listeners: Vec::new(),
        }
    }

    /// Add a listener callback for progress events.
    pub fn add_listener<L: ProgressListener + 'static>(&mut self, listener: L) {
        self.listeners.push(Arc::new(listener));
    }

    /// Record newly extracted bytes and notify listeners.
    pub fn record_bytes(&self, bytes: u64, current_file: &str) {
        self.bytes_completed.fetch_add(bytes, Ordering::Relaxed);
        self.notify(current_file);
    }

    /// Mark a file as completed.
    pub fn file_done(&self, current_file: &str) {
        self.files_completed.fetch_add(1, Ordering::Relaxed);
        self.notify(current_file);
    }

    /// Calculate instantaneous progress snapshot.
    pub fn snapshot(&self, current_file: &str) -> ProgressSnapshot {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let bytes_done = self.bytes_completed.load(Ordering::Relaxed);
        let files_done = self.files_completed.load(Ordering::Relaxed);

        let throughput = if elapsed > 0.001 {
            bytes_done as f64 / elapsed
        } else {
            0.0
        };

        let eta_seconds = if throughput > 1024.0 && self.total_bytes > bytes_done {
            let remaining_bytes = self.total_bytes - bytes_done;
            Some(remaining_bytes as f64 / throughput)
        } else {
            None
        };

        ProgressSnapshot {
            files_completed: files_done,
            total_files: self.total_files,
            bytes_completed: bytes_done,
            total_bytes: self.total_bytes,
            current_file: current_file.to_string(),
            throughput_bytes_per_sec: throughput,
            elapsed_seconds: elapsed,
            eta_seconds,
        }
    }

    fn notify(&self, current_file: &str) {
        if self.listeners.is_empty() {
            return;
        }
        let snap = self.snapshot(current_file);
        for listener in &self.listeners {
            listener.on_progress(&snap);
        }
    }
}
