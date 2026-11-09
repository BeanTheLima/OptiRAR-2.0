pub mod config;
pub mod error;
pub mod i18n;
pub mod queue;
pub mod traits;
pub mod types;

pub use config::{AppConfig, ThemeMode};
pub use error::{OptiError, Result};
pub use i18n::{get_i18n, t, t_format, I18nManager, Language};
pub use queue::{QueueItem, TaskKind, TaskQueue, TaskState};
pub use traits::{
    ArchiveReader, ArchiveWriter, CancellationToken, ProgressListener, ProgressSnapshot,
};
pub use types::{
    format_bytes, ArchiveEntry, ArchiveFormat, ArchiveMetadata, CompressionLevel,
    CompressionProfile, ExtractionOptions, OverwritePolicy,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_extensions() {
        assert_eq!(ArchiveFormat::from_extension("zip"), ArchiveFormat::Zip);
        assert_eq!(ArchiveFormat::from_extension("rar"), ArchiveFormat::Rar5);
        assert_eq!(ArchiveFormat::from_extension("7z"), ArchiveFormat::SevenZip);
        assert_eq!(ArchiveFormat::from_extension("tar"), ArchiveFormat::Tar);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_cancellation_token() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
        token.reset();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_progress_snapshot() {
        let snapshot = ProgressSnapshot {
            files_completed: 5,
            total_files: 10,
            bytes_completed: 50 * 1024 * 1024,
            total_bytes: 100 * 1024 * 1024,
            current_file: "test.txt".to_string(),
            throughput_bytes_per_sec: 10.0 * 1024.0 * 1024.0,
            elapsed_seconds: 5.0,
            eta_seconds: Some(5.0),
        };
        assert_eq!(snapshot.percent(), 50.0);
        assert_eq!(snapshot.display_throughput(), "10.0 MB/s");
        assert_eq!(snapshot.display_eta(), "5s remaining");
    }
}
