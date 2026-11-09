use optirar_core::ArchiveEntry;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ArchiveDiffResult {
    pub only_in_a: Vec<ArchiveEntry>,
    pub only_in_b: Vec<ArchiveEntry>,
    pub identical: Vec<ArchiveEntry>,
    pub modified: Vec<(ArchiveEntry, ArchiveEntry)>,
}

impl ArchiveDiffResult {
    pub fn is_identical(&self) -> bool {
        self.only_in_a.is_empty() && self.only_in_b.is_empty() && self.modified.is_empty()
    }

    pub fn total_differences(&self) -> usize {
        self.only_in_a.len() + self.only_in_b.len() + self.modified.len()
    }
}

pub struct ArchiveComparator;

impl ArchiveComparator {
    /// Compare two collections of archive entries.
    pub fn compare(entries_a: &[ArchiveEntry], entries_b: &[ArchiveEntry]) -> ArchiveDiffResult {
        let mut map_a: HashMap<String, &ArchiveEntry> = HashMap::new();
        for entry in entries_a {
            let normalized = entry.path.replace('\\', "/").trim_start_matches('/').to_string();
            map_a.insert(normalized, entry);
        }

        let mut map_b: HashMap<String, &ArchiveEntry> = HashMap::new();
        for entry in entries_b {
            let normalized = entry.path.replace('\\', "/").trim_start_matches('/').to_string();
            map_b.insert(normalized, entry);
        }

        let mut result = ArchiveDiffResult::default();

        // Check entries from A
        for (path_a, entry_a) in &map_a {
            if let Some(entry_b) = map_b.get(path_a) {
                let size_match = entry_a.uncompressed_size == entry_b.uncompressed_size;
                let crc_match = match (entry_a.crc32, entry_b.crc32) {
                    (Some(ca), Some(cb)) => ca == cb,
                    _ => true, // Fallback to size if CRC is missing
                };

                if size_match && crc_match {
                    result.identical.push((*entry_a).clone());
                } else {
                    result.modified.push(((*entry_a).clone(), (*entry_b).clone()));
                }
            } else {
                result.only_in_a.push((*entry_a).clone());
            }
        }

        // Check entries only in B
        for (path_b, entry_b) in &map_b {
            if !map_a.contains_key(path_b) {
                result.only_in_b.push((*entry_b).clone());
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(path: &str, size: u64, crc: u32) -> ArchiveEntry {
        ArchiveEntry {
            index: 0,
            path: path.to_string(),
            uncompressed_size: size,
            compressed_size: size / 2,
            crc32: Some(crc),
            is_directory: false,
            is_encrypted: false,
            hash_hex: None,
            modified_timestamp: None,
            windows_attributes: None,
            unix_mode: None,
        }
    }

    #[test]
    fn test_archive_diff() {
        let a = vec![
            make_entry("file1.txt", 100, 0x1111),
            make_entry("file2.txt", 200, 0x2222),
            make_entry("only_a.txt", 50, 0x3333),
        ];

        let b = vec![
            make_entry("file1.txt", 100, 0x1111), // identical
            make_entry("file2.txt", 250, 0x9999), // modified
            make_entry("only_b.txt", 80, 0x4444), // only in b
        ];

        let diff = ArchiveComparator::compare(&a, &b);
        assert_eq!(diff.identical.len(), 1);
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.only_in_a.len(), 1);
        assert_eq!(diff.only_in_b.len(), 1);
        assert!(!diff.is_identical());
    }
}
