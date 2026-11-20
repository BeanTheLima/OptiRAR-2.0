use optirar_core::traits::{ProgressListener, ProgressSnapshot};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

const MIN_NOTIFY_INTERVAL_MS: u64 = 25; // Cap progress callbacks to 40 Hz
const MIN_NOTIFY_BYTES_DELTA: u64 = 4 * 1024 * 1024; // Or at least 4 MB written

/// High-precision, lock-free progress tracker with throttled real-time throughput calculations.
#[derive(Clone)]
pub struct ProgressTracker {
    start_time: Instant,
    total_files: usize,
    total_bytes: u64,
    files_completed: Arc<AtomicUsize>,
    bytes_completed: Arc<AtomicU64>,
    last_notify_ms: Arc<AtomicU64>,
    last_notify_bytes: Arc<AtomicU64>,
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
            last_notify_ms: Arc::new(AtomicU64::new(0)),
            last_notify_bytes: Arc::new(AtomicU64::new(0)),
            listeners: Vec::new(),
        }
    }

    /// Add a listener callback for progress events.
    pub fn add_listener<L: ProgressListener + 'static>(&mut self, listener: L) {
        self.listeners.push(Arc::new(listener));
    }

    /// Record newly extracted bytes and notify listeners (rate-throttled to avoid thread contention).
    pub fn record_bytes(&self, bytes: u64, current_file: &str) {
        if bytes == 0 {
            return;
        }
        let new_bytes = self.bytes_completed.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if self.listeners.is_empty() {
            return;
        }

        let elapsed_ms = self.start_time.elapsed().as_millis() as u64;
        let last_ms = self.last_notify_ms.load(Ordering::Relaxed);
        let last_b = self.last_notify_bytes.load(Ordering::Relaxed);

        if new_bytes.saturating_sub(last_b) >= MIN_NOTIFY_BYTES_DELTA
            || elapsed_ms.saturating_sub(last_ms) >= MIN_NOTIFY_INTERVAL_MS
        {
            if self
                .last_notify_ms
                .compare_exchange(last_ms, elapsed_ms, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                self.last_notify_bytes.store(new_bytes, Ordering::Relaxed);
                self.notify(current_file);
            }
        }
    }

    /// Mark a file as completed.
    pub fn file_done(&self, current_file: &str) {
        self.files_completed.fetch_add(1, Ordering::Relaxed);
        if !self.listeners.is_empty() {
            let elapsed_ms = self.start_time.elapsed().as_millis() as u64;
            let last_ms = self.last_notify_ms.load(Ordering::Relaxed);
            if elapsed_ms.saturating_sub(last_ms) >= MIN_NOTIFY_INTERVAL_MS {
                let _ = self.last_notify_ms.compare_exchange(
                    last_ms,
                    elapsed_ms,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
                self.notify(current_file);
            }
        }
    }

    /// Final notification ensuring the 100% completion state is delivered.
    pub fn finish(&self, current_file: &str) {
        if !self.listeners.is_empty() {
            self.notify(current_file);
        }
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
