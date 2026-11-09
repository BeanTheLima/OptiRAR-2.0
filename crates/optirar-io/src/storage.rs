use std::path::Path;

/// Storage medium characteristics used to tune I/O buffer sizes and thread counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    /// Fast solid-state storage (NVMe / SATA SSD). High random IOPS, massive bandwidth.
    SolidStateDrive,
    /// Traditional spinning disk. High sequential throughput, high seek latency.
    MechanicalDrive,
    /// Network attached storage / UNC path (`\\server\share`). Latency-sensitive.
    NetworkShare,
    /// Default / generic storage.
    Generic,
}

impl StorageType {
    /// Detect storage characteristics from a filesystem path.
    pub fn detect_from_path<P: AsRef<Path>>(path: P) -> Self {
        let path_str = path.as_ref().to_string_lossy();

        // Check for UNC network path
        if path_str.starts_with(r"\\") || path_str.starts_with("//") {
            return StorageType::NetworkShare;
        }

        // On Windows, local drives (C:, D:, etc.) default to SSD profile on modern systems
        StorageType::SolidStateDrive
    }

    /// Optimal buffer size for this storage type in bytes.
    pub fn optimal_buffer_size(&self) -> usize {
        match self {
            StorageType::SolidStateDrive => 512 * 1024, // 512 KB
            StorageType::MechanicalDrive => 1024 * 1024, // 1 MB (reduces head seeks)
            StorageType::NetworkShare => 256 * 1024,    // 256 KB
            StorageType::Generic => 256 * 1024,
        }
    }

    /// Recommended maximum concurrent file writes to avoid disk queue starvation.
    pub fn max_concurrent_writers(&self, num_cpus: usize) -> usize {
        match self {
            StorageType::SolidStateDrive => num_cpus.max(4),
            StorageType::MechanicalDrive => 2, // Avoid thrashing disk head
            StorageType::NetworkShare => 4,
            StorageType::Generic => num_cpus.min(4),
        }
    }
}
