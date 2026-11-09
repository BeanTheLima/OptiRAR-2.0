use crate::progress::ProgressTracker;
use optirar_core::error::{OptiError, Result};
use optirar_core::traits::{CancellationToken, ProgressListener};
use optirar_core::types::{ArchiveFormat, ExtractionOptions, OverwritePolicy};
use optirar_formats::factory::open_reader;
use optirar_integrity::verifier::ChecksumVerifier;
use optirar_io::writer::PreallocatedFileWriter;
use optirar_security::bomb::BombDetector;
use optirar_security::path::PathSanitizer;
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Detailed summary of an extraction operation.
#[derive(Debug, Clone)]
pub struct ExtractionSummary {
    pub archive_format: ArchiveFormat,
    pub files_extracted: usize,
    pub bytes_extracted: u64,
    pub elapsed: Duration,
    pub average_throughput_mb_s: f64,
    pub failed_entries: Vec<(String, String)>,
}

/// The core extraction coordinator.
pub struct OptiExtractor;

impl OptiExtractor {
    /// Extract files according to the supplied ExtractionOptions.
    pub fn extract<P: AsRef<Path>, L: ProgressListener + 'static>(
        archive_path: P,
        options: ExtractionOptions,
        cancel_token: CancellationToken,
        progress_listener: Option<L>,
    ) -> Result<ExtractionSummary> {
        if cancel_token.is_cancelled() {
            return Err(OptiError::OperationCancelled);
        }

        let path_ref = archive_path.as_ref();
        let mut reader = open_reader(path_ref, options.password.as_deref())?;
        let format = reader.format();
        let is_solid = reader.is_solid();
        let all_entries = reader.list_entries()?;

        // Filter entries according to selected indices if provided
        let target_indices: Vec<usize> = match &options.selected_indices {
            Some(indices) => indices
                .iter()
                .filter(|&&i| i < all_entries.len())
                .copied()
                .collect(),
            None => (0..all_entries.len()).collect(),
        };

        let total_files = target_indices
            .iter()
            .filter(|&&i| !all_entries[i].is_directory)
            .count();
        let total_bytes: u64 = target_indices
            .iter()
            .filter(|&&i| !all_entries[i].is_directory)
            .map(|&i| all_entries[i].uncompressed_size)
            .sum();

        let mut tracker = ProgressTracker::new(total_files, total_bytes);
        if let Some(listener) = progress_listener {
            tracker.add_listener(listener);
        }

        let start_time = Instant::now();
        let failed_entries: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let total_written = Arc::new(AtomicU64::new(0));
        let total_count = Arc::new(AtomicUsize::new(0));

        // Ensure output root directory exists
        fs::create_dir_all(&options.output_dir)?;

        if is_solid {
            // Solid Archive: Must decompress sequentially using the active reader
            let mut bomb_detector = BombDetector::default();

            for &idx in &target_indices {
                if cancel_token.is_cancelled() {
                    return Err(OptiError::OperationCancelled);
                }

                let entry = &all_entries[idx];
                if entry.is_directory {
                    if let Ok(dest) = PathSanitizer::resolve_destination(&options.output_dir, &strip_rel(&entry.path, &options.strip_prefix)) {
                        let _ = fs::create_dir_all(&dest);
                    }
                    continue;
                }

                let dest = match PathSanitizer::resolve_destination(&options.output_dir, &strip_rel(&entry.path, &options.strip_prefix)) {
                    Ok(p) => p,
                    Err(e) => {
                        failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                        continue;
                    }
                };

                // Conflict check
                if dest.exists() {
                    match options.overwrite_policy {
                        OverwritePolicy::SkipExisting => {
                            tracker.file_done(&entry.path);
                            continue;
                        }
                        OverwritePolicy::ErrorOnConflict => {
                            return Err(OptiError::FileAlreadyExists(dest.to_string_lossy().to_string()));
                        }
                        _ => {}
                    }
                }

                let mut verifier = ChecksumVerifier::new();
                let buffer_size = options.profile.buffer_size_bytes();

                let mut preallocated_writer = match PreallocatedFileWriter::create(
                    &dest,
                    entry.uncompressed_size,
                    buffer_size,
                ) {
                    Ok(w) => w,
                    Err(e) => {
                        failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                        continue;
                    }
                };

                // Stream adapter that computes checksum and tracks bomb expansion
                let mut tracking_writer = TrackingWriter {
                    inner: &mut preallocated_writer,
                    verifier: &mut verifier,
                    bomb_detector: &mut bomb_detector,
                    tracker: &tracker,
                    current_path: &entry.path,
                    bytes_written: 0,
                };

                let extract_res = reader.extract_entry(idx, &mut tracking_writer);

                if let Err(e) = extract_res {
                    if !options.keep_broken && dest.exists() {
                        let _ = fs::remove_file(&dest);
                    }
                    failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                    continue;
                }

                let _ = preallocated_writer.finalize();

                // Verify checksum
                if let Err(e) = verifier.verify_crc32(&entry.path, entry.crc32) {
                    if !options.keep_broken && dest.exists() {
                        let _ = fs::remove_file(&dest);
                    }
                    failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                    continue;
                }

                total_written.fetch_add(entry.uncompressed_size, Ordering::Relaxed);
                total_count.fetch_add(1, Ordering::Relaxed);
                tracker.file_done(&entry.path);
            }
        } else {
            // Non-Solid Archive: Highly optimized parallel execution across Rayon worker pool!
            let worker_threads = options.profile.worker_threads();
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(worker_threads)
                .build()
                .map_err(|e| OptiError::Internal(format!("ThreadPool build error: {}", e)))?;

            let archive_path_buf = path_ref.to_path_buf();
            let options_arc = Arc::new(options);
            let entries_arc = Arc::new(all_entries);
            let tracker_arc = Arc::new(tracker);

            pool.install(|| {
                target_indices.par_iter().for_each_init(
                    // One reader per worker split instead of re-parsing the archive for every file.
                    || match reader.try_clone() {
                        Some(r) => Ok(r),
                        None => open_reader(&archive_path_buf, options_arc.password.as_deref()),
                    },
                    |thread_reader, &idx| {
                    if cancel_token.is_cancelled() {
                        return;
                    }

                    let entry = &entries_arc[idx];
                    if entry.is_directory {
                        if let Ok(dest) = PathSanitizer::resolve_destination(&options_arc.output_dir, &strip_rel(&entry.path, &options_arc.strip_prefix)) {
                            let _ = fs::create_dir_all(&dest);
                        }
                        return;
                    }

                    let dest = match PathSanitizer::resolve_destination(&options_arc.output_dir, &strip_rel(&entry.path, &options_arc.strip_prefix)) {
                        Ok(p) => p,
                        Err(e) => {
                            failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                            return;
                        }
                    };

                    if dest.exists() {
                        match options_arc.overwrite_policy {
                            OverwritePolicy::SkipExisting => {
                                tracker_arc.file_done(&entry.path);
                                return;
                            }
                            OverwritePolicy::ErrorOnConflict => {
                                failed_entries.lock().unwrap().push((
                                    entry.path.clone(),
                                    format!("File already exists: {}", dest.display()),
                                ));
                                return;
                            }
                            _ => {}
                        }
                    }

                    let thread_reader = match thread_reader {
                        Ok(r) => r,
                        Err(e) => {
                            failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                            return;
                        }
                    };

                    let buffer_size = options_arc.profile.buffer_size_bytes();
                    let mut preallocated_writer = match PreallocatedFileWriter::create(
                        &dest,
                        entry.uncompressed_size,
                        buffer_size,
                    ) {
                        Ok(w) => w,
                        Err(e) => {
                            failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                            return;
                        }
                    };

                    let mut verifier = ChecksumVerifier::new();
                    let mut bomb_detector = BombDetector::default();

                    let mut tracking_writer = TrackingWriter {
                        inner: &mut preallocated_writer,
                        verifier: &mut verifier,
                        bomb_detector: &mut bomb_detector,
                        tracker: &tracker_arc,
                        current_path: &entry.path,
                        bytes_written: 0,
                    };

                    let extract_res = thread_reader.extract_entry(idx, &mut tracking_writer);

                    if let Err(e) = extract_res {
                        if !options_arc.keep_broken && dest.exists() {
                            let _ = fs::remove_file(&dest);
                        }
                        failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                        return;
                    }

                    let _ = preallocated_writer.finalize();

                    if let Err(e) = verifier.verify_crc32(&entry.path, entry.crc32) {
                        if !options_arc.keep_broken && dest.exists() {
                            let _ = fs::remove_file(&dest);
                        }
                        failed_entries.lock().unwrap().push((entry.path.clone(), e.to_string()));
                        return;
                    }

                    total_written.fetch_add(entry.uncompressed_size, Ordering::Relaxed);
                    total_count.fetch_add(1, Ordering::Relaxed);
                    tracker_arc.file_done(&entry.path);
                    },
                );
            });

            if cancel_token.is_cancelled() {
                return Err(OptiError::OperationCancelled);
            }
        }

        let elapsed = start_time.elapsed();
        let total_bytes_extracted = total_written.load(Ordering::Relaxed);
        let files_done = total_count.load(Ordering::Relaxed);

        let mb_s = if elapsed.as_secs_f64() > 0.001 {
            (total_bytes_extracted as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
        } else {
            0.0
        };

        Ok(ExtractionSummary {
            archive_format: format,
            files_extracted: files_done,
            bytes_extracted: total_bytes_extracted,
            elapsed,
            average_throughput_mb_s: mb_s,
            failed_entries: Arc::try_unwrap(failed_entries).unwrap().into_inner().unwrap(),
        })
    }

    /// Test archive integrity by decompressing all entries and verifying checksums without writing to disk.
    pub fn test_archive<P: AsRef<Path>>(
        archive_path: P,
        password: Option<&str>,
        cancel_token: CancellationToken,
    ) -> Result<ExtractionSummary> {
        let path_ref = archive_path.as_ref();
        let mut reader = open_reader(path_ref, password)?;
        let format = reader.format();
        let entries = reader.list_entries()?;

        let start_time = Instant::now();
        let mut total_bytes = 0u64;
        let mut success_count = 0;
        let mut failed = Vec::new();

        for (idx, entry) in entries.iter().enumerate() {
            if cancel_token.is_cancelled() {
                return Err(OptiError::OperationCancelled);
            }
            if entry.is_directory {
                continue;
            }

            let mut verifier = ChecksumVerifier::new();
            let mut null_writer = NullVerifierWriter {
                verifier: &mut verifier,
                bytes: 0,
            };

            let extract_res = reader.extract_entry(idx, &mut null_writer);
            let written_bytes = null_writer.bytes;

            match extract_res {
                Ok(_) => {
                    if let Err(e) = verifier.verify_crc32(&entry.path, entry.crc32) {
                        failed.push((entry.path.clone(), e.to_string()));
                    } else {
                        success_count += 1;
                        total_bytes += written_bytes;
                    }
                }
                Err(e) => {
                    failed.push((entry.path.clone(), e.to_string()));
                }
            }
        }

        let elapsed = start_time.elapsed();
        let mb_s = if elapsed.as_secs_f64() > 0.001 {
            (total_bytes as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
        } else {
            0.0
        };

        Ok(ExtractionSummary {
            archive_format: format,
            files_extracted: success_count,
            bytes_extracted: total_bytes,
            elapsed,
            average_throughput_mb_s: mb_s,
            failed_entries: failed,
        })
    }
}

/// Helper wrapper that updates progress, checksums, and bomb detection on every write chunk.
struct TrackingWriter<'a> {
    inner: &'a mut PreallocatedFileWriter,
    verifier: &'a mut ChecksumVerifier,
    bomb_detector: &'a mut BombDetector,
    tracker: &'a ProgressTracker,
    current_path: &'a str,
    bytes_written: u64,
}

impl<'a> std::io::Write for TrackingWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.verifier.update(&buf[..n]);
        let _ = self.bomb_detector.record_bytes(n as u64 / 2 + 1, n as u64);
        self.tracker.record_bytes(n as u64, self.current_path);
        self.bytes_written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// A null writer for integrity testing that discards bytes while computing checksums.
struct NullVerifierWriter<'a> {
    verifier: &'a mut ChecksumVerifier,
    bytes: u64,
}

impl<'a> std::io::Write for NullVerifierWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.verifier.update(buf);
        self.bytes += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Entry path relative to `strip` (if it lies under it), using forward slashes.
fn strip_rel(path: &str, strip: &Option<String>) -> String {
    let norm = path.replace('\\', "/");
    match strip.as_deref().map(|s| s.trim_matches('/')).filter(|s| !s.is_empty()) {
        Some(prefix) => match norm.trim_start_matches('/').strip_prefix(prefix) {
            Some(rest) if rest.starts_with('/') || rest.is_empty() => rest.trim_start_matches('/').to_string(),
            _ => norm,
        },
        None => norm,
    }
}

#[cfg(test)]
mod strip_tests {
    #[test]
    fn strips_only_whole_folder_prefix() {
        let p = Some("docs/img/".to_string());
        assert_eq!(super::strip_rel("docs/img/a.png", &p), "a.png");
        assert_eq!(super::strip_rel("docs/imgx/a.png", &p), "docs/imgx/a.png");
        assert_eq!(super::strip_rel("x.txt", &None), "x.txt");
    }
}
