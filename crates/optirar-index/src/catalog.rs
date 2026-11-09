use optirar_core::types::{ArchiveEntry, ArchiveFormat, ArchiveMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An indexed catalog of all entries inside an archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveCatalog {
    pub format: ArchiveFormat,
    pub entries: Vec<ArchiveEntry>,
    pub total_uncompressed_bytes: u64,
    pub total_compressed_bytes: u64,
    pub file_count: usize,
    pub dir_count: usize,
    pub is_solid: bool,
    path_index: HashMap<String, usize>,
}

impl ArchiveCatalog {
    /// Build catalog from a list of archive entries.
    pub fn new(format: ArchiveFormat, entries: Vec<ArchiveEntry>, is_solid: bool) -> Self {
        let mut total_uncompressed = 0u64;
        let mut total_compressed = 0u64;
        let mut file_count = 0;
        let mut dir_count = 0;
        let mut path_index = HashMap::with_capacity(entries.len());

        for (idx, entry) in entries.iter().enumerate() {
            total_uncompressed += entry.uncompressed_size;
            total_compressed += entry.compressed_size;
            if entry.is_directory {
                dir_count += 1;
            } else {
                file_count += 1;
            }
            path_index.insert(entry.path.clone(), idx);
        }

        Self {
            format,
            entries,
            total_uncompressed_bytes: total_uncompressed,
            total_compressed_bytes: total_compressed,
            file_count,
            dir_count,
            is_solid,
            path_index,
        }
    }

    /// Total number of entries (files + directories).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Lookup entry by index.
    pub fn get_by_index(&self, index: usize) -> Option<&ArchiveEntry> {
        self.entries.get(index)
    }

    /// Lookup entry index by exact normalized relative path.
    pub fn find_by_path(&self, path: &str) -> Option<&ArchiveEntry> {
        self.path_index.get(path).and_then(|idx| self.entries.get(*idx))
    }

    /// Overall archive metadata summary.
    pub fn metadata(&self) -> ArchiveMetadata {
        ArchiveMetadata {
            format: self.format,
            total_entries: self.entries.len(),
            file_count: self.file_count,
            dir_count: self.dir_count,
            uncompressed_size: self.total_uncompressed_bytes,
            compressed_size: self.total_compressed_bytes,
            is_solid: self.is_solid,
            is_encrypted: self.entries.iter().any(|e| e.is_encrypted),
            comment: None,
        }
    }
}
