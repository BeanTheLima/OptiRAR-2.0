use optirar_core::ArchiveEntry;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateGroup {
    pub uncompressed_size: u64,
    pub crc32: Option<u32>,
    pub entries: Vec<ArchiveEntry>,
    pub wasted_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DuplicateReport {
    pub groups: Vec<DuplicateGroup>,
    pub total_duplicate_files: usize,
    pub total_wasted_bytes: u64,
}

pub struct DuplicateFinder;

impl DuplicateFinder {
    pub fn find_duplicates(entries: &[ArchiveEntry]) -> DuplicateReport {
        // Group by (size, crc32)
        let mut map: HashMap<(u64, Option<u32>), Vec<ArchiveEntry>> = HashMap::new();

        for entry in entries {
            if entry.is_directory || entry.uncompressed_size == 0 {
                continue;
            }
            map.entry((entry.uncompressed_size, entry.crc32))
                .or_default()
                .push(entry.clone());
        }

        let mut groups = Vec::new();
        let mut total_duplicate_files = 0;
        let mut total_wasted_bytes = 0u64;

        for ((size, crc), group_entries) in map {
            if group_entries.len() > 1 {
                let duplicate_count = group_entries.len() - 1;
                let wasted = (duplicate_count as u64) * size;

                total_duplicate_files += duplicate_count;
                total_wasted_bytes += wasted;

                groups.push(DuplicateGroup {
                    uncompressed_size: size,
                    crc32: crc,
                    entries: group_entries,
                    wasted_bytes: wasted,
                });
            }
        }

        // Sort descending by wasted bytes
        groups.sort_by(|a, b| b.wasted_bytes.cmp(&a.wasted_bytes));

        DuplicateReport {
            groups,
            total_duplicate_files,
            total_wasted_bytes,
        }
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
    fn test_duplicate_detection() {
        let entries = vec![
            make_entry("images/logo1.png", 5000, 0x1234),
            make_entry("backup/logo1.png", 5000, 0x1234), // dup 1
            make_entry("icons/logo1.png", 5000, 0x1234),  // dup 2
            make_entry("unique.txt", 100, 0x9999),
        ];

        let report = DuplicateFinder::find_duplicates(&entries);
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.total_duplicate_files, 2);
        assert_eq!(report.total_wasted_bytes, 10000);
    }
}
