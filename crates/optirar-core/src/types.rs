use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Supported archive formats in OptiRAR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArchiveFormat {
    Zip,
    Tar,
    Gzip,
    Zstd,
    SevenZip,
    Rar4,
    Rar5,
    Unknown,
}

impl ArchiveFormat {
    /// Human-readable format name.
    pub fn name(&self) -> &'static str {
        match self {
            ArchiveFormat::Zip => "ZIP",
            ArchiveFormat::Tar => "TAR",
            ArchiveFormat::Gzip => "GZIP",
            ArchiveFormat::Zstd => "Zstandard",
            ArchiveFormat::SevenZip => "7-Zip",
            ArchiveFormat::Rar4 => "RAR 4.x",
            ArchiveFormat::Rar5 => "RAR 5.x",
            ArchiveFormat::Unknown => "Unknown",
        }
    }

    /// Common file extensions for this format.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            ArchiveFormat::Zip => &["zip", "cbz", "jar"],
            ArchiveFormat::Tar => &["tar", "cbt"],
            ArchiveFormat::Gzip => &["gz", "tgz", "tar.gz"],
            ArchiveFormat::Zstd => &["zst", "tzst", "tar.zst"],
            ArchiveFormat::SevenZip => &["7z", "cb7"],
            ArchiveFormat::Rar4 | ArchiveFormat::Rar5 => &["rar", "cbr"],
            ArchiveFormat::Unknown => &[],
        }
    }

    /// Whether this format legally and technically supports creation/writing in OptiRAR.
    pub fn can_create(&self) -> bool {
        match self {
            ArchiveFormat::Zip
            | ArchiveFormat::Tar
            | ArchiveFormat::Gzip
            | ArchiveFormat::Zstd
            | ArchiveFormat::SevenZip => true,
            ArchiveFormat::Rar4 | ArchiveFormat::Rar5 | ArchiveFormat::Unknown => false,
        }
    }

    /// Whether this format supports extraction in OptiRAR.
    pub fn can_extract(&self) -> bool {
        match self {
            ArchiveFormat::Unknown => false,
            _ => true,
        }
    }

    /// Guess format from file extension.
    pub fn from_extension(ext: &str) -> Self {
        let ext_lower = ext.to_ascii_lowercase();
        match ext_lower.as_str() {
            "zip" | "cbz" | "jar" => ArchiveFormat::Zip,
            "tar" | "cbt" => ArchiveFormat::Tar,
            "gz" | "tgz" => ArchiveFormat::Gzip,
            "zst" | "tzst" => ArchiveFormat::Zstd,
            "7z" | "cb7" => ArchiveFormat::SevenZip,
            "rar" | "cbr" => ArchiveFormat::Rar5, // default to RAR5 until magic bytes parsed
            _ => ArchiveFormat::Unknown,
        }
    }

    /// Guess format from path.
    pub fn from_path(path: &Path) -> Self {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let lower = filename.to_ascii_lowercase();
        if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            return ArchiveFormat::Gzip;
        }
        if lower.ends_with(".tar.zst") || lower.ends_with(".tzst") {
            return ArchiveFormat::Zstd;
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            Self::from_extension(ext)
        } else {
            ArchiveFormat::Unknown
        }
    }
}

/// A single entry (file or folder) within an archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    /// Zero-based sequential entry index in the archive.
    pub index: usize,
    /// Normalized relative path (forward slashes, stripped drive letters and leading slashes).
    pub path: String,
    /// Uncompressed size in bytes (64-bit to prevent overflow on >4GB files).
    pub uncompressed_size: u64,
    /// Compressed size in bytes within the archive.
    pub compressed_size: u64,
    /// True if entry is a directory.
    pub is_directory: bool,
    /// True if entry data is encrypted with a password.
    pub is_encrypted: bool,
    /// CRC32 checksum if provided by the archive headers.
    pub crc32: Option<u32>,
    /// BLAKE3 or BLAKE2sp cryptographic hash string if available.
    pub hash_hex: Option<String>,
    /// Last modified time in UNIX seconds.
    pub modified_timestamp: Option<i64>,
    /// Windows file attributes (e.g. readonly, hidden, system).
    pub windows_attributes: Option<u32>,
    /// Unix file permission mode bits.
    pub unix_mode: Option<u32>,
}

impl ArchiveEntry {
    /// Calculate compression ratio as a percentage (e.g. 45.2% compressed).
    pub fn compression_ratio(&self) -> f64 {
        if self.uncompressed_size == 0 {
            0.0
        } else {
            let ratio = (self.compressed_size as f64 / self.uncompressed_size as f64) * 100.0;
            ratio.min(100.0)
        }
    }

    /// Human-friendly uncompressed size string (e.g. "14.2 MB").
    pub fn display_size(&self) -> String {
        format_bytes(self.uncompressed_size)
    }

    /// Human-friendly compressed size string.
    pub fn display_compressed_size(&self) -> String {
        format_bytes(self.compressed_size)
    }

    /// Filename without parent directory path.
    pub fn file_name(&self) -> &str {
        Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.path)
    }
}

/// Helper function to format byte counts into human-readable strings.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Configurable performance profile for extraction and compression operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionProfile {
    /// Eco: Low memory consumption and conservative thread usage. Ideal for background or constrained systems.
    Eco,
    /// Balanced: Optimal throughput and resource utilization for typical desktop usage.
    Balanced,
    /// Performance: Maximizes all CPU cores, aggressive buffer pre-allocation, highest I/O throughput.
    Performance,
    /// Custom configuration.
    Custom {
        worker_threads: usize,
        buffer_size_kb: usize,
    },
}

impl CompressionProfile {
    /// Recommended worker thread count.
    pub fn worker_threads(&self) -> usize {
        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        match self {
            CompressionProfile::Eco => (num_cpus / 2).max(1),
            CompressionProfile::Balanced => num_cpus.max(2),
            CompressionProfile::Performance => (num_cpus * 2).max(4),
            CompressionProfile::Custom { worker_threads, .. } => *worker_threads,
        }
    }

    /// Buffer size in bytes for streaming I/O.
    pub fn buffer_size_bytes(&self) -> usize {
        match self {
            CompressionProfile::Eco => 64 * 1024,          // 64 KB
            CompressionProfile::Balanced => 256 * 1024,     // 256 KB
            CompressionProfile::Performance => 1024 * 1024, // 1 MB
            CompressionProfile::Custom { buffer_size_kb, .. } => buffer_size_kb * 1024,
        }
    }
}

/// Compression level for creating archives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionLevel {
    Store,   // Level 0: No compression, fastest packaging
    Fastest, // Level 1: Minimal compression, high speed
    Normal,  // Level 5-6: Balanced ratio and speed
    Maximum, // Level 9: Highest compression ratio
}

impl Default for CompressionLevel {
    fn default() -> Self {
        CompressionLevel::Normal
    }
}

/// Conflict resolution policy when writing extracted files to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverwritePolicy {
    /// Overwrite existing destination files unconditionally.
    Overwrite,
    /// Skip files that already exist on disk.
    SkipExisting,
    /// Rename newly extracted file by appending a counter (e.g. `file (1).txt`).
    RenameNew,
    /// Abort with an error if destination file already exists.
    ErrorOnConflict,
}

impl Default for OverwritePolicy {
    fn default() -> Self {
        OverwritePolicy::Overwrite
    }
}

/// Detailed options for extracting an archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionOptions {
    /// Destination directory on disk.
    pub output_dir: PathBuf,
    /// Overwrite policy for conflicting files.
    pub overwrite_policy: OverwritePolicy,
    /// Resource usage profile.
    pub profile: CompressionProfile,
    /// Specific entry indices to extract. If None, extracts all entries.
    pub selected_indices: Option<Vec<usize>>,
    /// Preserve Windows file attributes (e.g. readonly, hidden).
    pub preserve_attributes: bool,
    /// Preserve modification and creation timestamps.
    pub preserve_timestamps: bool,
    /// Decryption password if the archive is encrypted.
    pub password: Option<String>,
    /// Keep partially extracted files on failure instead of deleting them.
    pub keep_broken: bool,
    /// Archive folder (e.g. "docs/img/") removed from entry paths on extraction, like WinRAR extracting from a subfolder.
    pub strip_prefix: Option<String>,
}

impl Default for ExtractionOptions {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("."),
            overwrite_policy: OverwritePolicy::Overwrite,
            profile: CompressionProfile::Balanced,
            selected_indices: None,
            preserve_attributes: true,
            preserve_timestamps: true,
            password: None,
            keep_broken: false,
            strip_prefix: None,
        }
    }
}

/// High-level archive metadata summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    pub format: ArchiveFormat,
    pub total_entries: usize,
    pub file_count: usize,
    pub dir_count: usize,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub is_solid: bool,
    pub is_encrypted: bool,
    pub comment: Option<String>,
}

impl ArchiveMetadata {
    pub fn overall_ratio(&self) -> f64 {
        if self.uncompressed_size == 0 {
            0.0
        } else {
            (self.compressed_size as f64 / self.uncompressed_size as f64) * 100.0
        }
    }
}
