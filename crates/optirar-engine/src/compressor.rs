use crate::progress::ProgressTracker;
use optirar_core::error::{OptiError, Result};
use optirar_core::traits::{CancellationToken, ProgressListener};
use optirar_core::types::{ArchiveFormat, CompressionLevel};
use optirar_formats::create_writer_with_password;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Summary of an archive creation operation.
#[derive(Debug, Clone)]
pub struct CompressionSummary {
    pub archive_format: ArchiveFormat,
    pub files_added: usize,
    pub bytes_compressed: u64,
    pub archive_size_bytes: u64,
    pub elapsed: Duration,
    pub average_throughput_mb_s: f64,
}

/// The core compression coordinator.
pub struct OptiCompressor;

impl OptiCompressor {
    /// Create a new archive from the specified source paths.
    pub fn create_archive<P: AsRef<Path>, L: ProgressListener + 'static>(
        destination: P,
        sources: &[PathBuf],
        format: ArchiveFormat,
        level: CompressionLevel,
        cancel_token: CancellationToken,
        progress_listener: Option<L>,
    ) -> Result<CompressionSummary> {
        Self::create_archive_with_password(destination, sources, format, level, None, cancel_token, progress_listener)
    }

    /// Like [`Self::create_archive`], encrypting ZIP (AES-256) / 7z contents when `password` is set.
    pub fn create_archive_with_password<P: AsRef<Path>, L: ProgressListener + 'static>(
        destination: P,
        sources: &[PathBuf],
        format: ArchiveFormat,
        level: CompressionLevel,
        password: Option<&str>,
        cancel_token: CancellationToken,
        progress_listener: Option<L>,
    ) -> Result<CompressionSummary> {
        let dest_ref = destination.as_ref();

        // Enforce licensing: RAR creation is strictly prohibited
        if !format.can_create() {
            return Err(OptiError::CreationNotSupported(format));
        }

        let start_time = Instant::now();
        let mut file_list: Vec<(String, PathBuf, u64)> = Vec::new();

        for source in sources {
            collect_files_recursive(source, "", &mut file_list)?;
        }

        let total_files = file_list.len();
        let total_bytes: u64 = file_list.iter().map(|(_, _, size)| *size).sum();

        let mut tracker = ProgressTracker::new(total_files, total_bytes);
        if let Some(listener) = progress_listener {
            tracker.add_listener(listener);
        }

        let mut writer = create_writer_with_password(dest_ref, format, level, password)?;
        let mut files_added = 0;
        let mut bytes_processed = 0u64;

        for (archive_path, source_path, file_size) in file_list {
            if cancel_token.is_cancelled() {
                // Remove incomplete archive on cancellation
                let _ = fs::remove_file(dest_ref);
                return Err(OptiError::OperationCancelled);
            }

            writer.add_file(&archive_path, &source_path)?;
            files_added += 1;
            bytes_processed += file_size;
            tracker.record_bytes(file_size, &archive_path);
            tracker.file_done(&archive_path);
        }

        writer.finish()?;

        let elapsed = start_time.elapsed();
        let archive_size = dest_ref.metadata().map(|m| m.len()).unwrap_or(0);

        let mb_s = if elapsed.as_secs_f64() > 0.001 {
            (bytes_processed as f64 / (1024.0 * 1024.0)) / elapsed.as_secs_f64()
        } else {
            0.0
        };

        Ok(CompressionSummary {
            archive_format: format,
            files_added,
            bytes_compressed: bytes_processed,
            archive_size_bytes: archive_size,
            elapsed,
            average_throughput_mb_s: mb_s,
        })
    }
}

/// Recursively collect all files and directory relative paths.
fn collect_files_recursive(
    current_path: &Path,
    relative_prefix: &str,
    collected: &mut Vec<(String, PathBuf, u64)>,
) -> Result<()> {
    let file_name = current_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let entry_path = if relative_prefix.is_empty() {
        file_name
    } else {
        format!("{}/{}", relative_prefix, file_name)
    };

    if current_path.is_dir() {
        for entry in fs::read_dir(current_path)? {
            let entry = entry?;
            let path = entry.path();
            collect_files_recursive(&path, &entry_path, collected)?;
        }
    } else {
        let size = current_path.metadata().map(|m| m.len()).unwrap_or(0);
        collected.push((entry_path, current_path.to_path_buf(), size));
    }

    Ok(())
}
