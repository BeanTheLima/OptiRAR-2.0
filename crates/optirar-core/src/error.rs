use crate::types::ArchiveFormat;
use thiserror::Error;

/// Comprehensive error type for OptiRAR operations.
#[derive(Debug, Error)]
pub enum OptiError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Format not supported for this operation: {0:?}")]
    FormatUnsupported(ArchiveFormat),

    #[error("Creation of {0:?} archives is not supported or prohibited by license")]
    CreationNotSupported(ArchiveFormat),

    #[error("Malformed or corrupt archive: {0}")]
    MalformedArchive(String),

    #[error("Checksum mismatch for '{file}': expected {expected}, computed {actual}")]
    ChecksumMismatch {
        file: String,
        expected: String,
        actual: String,
    },

    #[error("Security violation: Path traversal or Zip Slip attempt detected in '{0}'")]
    PathTraversalDetected(String),

    #[error("Security violation: Archive bomb detected (compression ratio: {ratio:.1}:1, size: {uncompressed_size} bytes)")]
    ArchiveBombDetected {
        ratio: f64,
        uncompressed_size: u64,
    },

    #[error("Archive is encrypted. A password is required to decrypt entries.")]
    PasswordRequired,

    #[error("Incorrect password provided for encrypted archive.")]
    InvalidPassword,

    #[error("Operation was cancelled by user")]
    OperationCancelled,

    #[error("Entry '{0}' was not found in archive index")]
    EntryNotFound(String),

    #[error("Destination file already exists and overwrite policy is ErrorOnConflict: '{0}'")]
    FileAlreadyExists(String),

    #[error("Multi-volume archive missing required volume: '{0}'")]
    MissingVolume(String),

    #[error("Internal engine error: {0}")]
    Internal(String),
}

/// Standard Result type alias for OptiRAR.
pub type Result<T> = std::result::Result<T, OptiError>;
