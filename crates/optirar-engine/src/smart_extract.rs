use optirar_core::ArchiveEntry;
use std::path::{Path, PathBuf};

/// Smart Extraction destination calculator.
///
/// If an archive's contents already reside in a single top-level directory,
/// extracting into a subfolder with the archive name would create redundant
/// nested directories (e.g. `MyProject/MyProject/...`).
///
/// Smart Extraction avoids this by detecting top-level entry distribution:
/// - If all entries share a common top-level directory, extract directly to `destination_base`.
/// - If entries contain multiple files/folders at the root level, extract into `destination_base / <archive_stem>`.
pub fn compute_smart_destination<P: AsRef<Path>>(
    archive_path: P,
    destination_base: P,
    entries: &[ArchiveEntry],
) -> PathBuf {
    let archive_path = archive_path.as_ref();
    let destination_base = destination_base.as_ref();

    if entries.is_empty() {
        return destination_base.to_path_buf();
    }

    // Identify distinct top-level components
    let mut top_level_roots = std::collections::HashSet::new();

    for entry in entries {
        // Normalize path separators to forward slash
        let clean_path = entry.path.replace('\\', "/");
        let trimmed = clean_path.trim_start_matches('/');
        if trimmed.is_empty() {
            continue;
        }

        let first_component = match trimmed.split_once('/') {
            Some((root, _)) => root,
            None => trimmed,
        };

        top_level_roots.insert(first_component.to_string());
        if top_level_roots.len() > 1 {
            break;
        }
    }

    // If there is strictly 1 top-level root AND it's a directory (or the only root)
    if top_level_roots.len() == 1 {
        // Safe to extract directly into destination_base without creating a subfolder
        destination_base.to_path_buf()
    } else {
        // Multiple items at root level: enclose in a dedicated folder named after archive stem
        let stem = archive_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Extracted");
        destination_base.join(stem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(path: &str, is_dir: bool) -> ArchiveEntry {
        ArchiveEntry {
            index: 0,
            path: path.to_string(),
            uncompressed_size: 100,
            compressed_size: 50,
            crc32: Some(1234),
            is_directory: is_dir,
            is_encrypted: false,
            hash_hex: None,
            modified_timestamp: None,
            windows_attributes: None,
            unix_mode: None,
        }
    }

    #[test]
    fn test_single_root_folder_does_not_nest() {
        let entries = vec![
            make_entry("my_app/index.js", false),
            make_entry("my_app/package.json", false),
            make_entry("my_app/src/main.rs", false),
        ];

        let dest = compute_smart_destination(
            Path::new("my_app.zip"),
            Path::new("C:/Users/Downloads"),
            &entries,
        );

        assert_eq!(dest, PathBuf::from("C:/Users/Downloads"));
    }

    #[test]
    fn test_multi_root_items_creates_folder() {
        let entries = vec![
            make_entry("file1.txt", false),
            make_entry("file2.txt", false),
            make_entry("subfolder/data.json", false),
        ];

        let dest = compute_smart_destination(
            Path::new("data_archive.zip"),
            Path::new("C:/Users/Downloads"),
            &entries,
        );

        assert_eq!(dest, PathBuf::from("C:/Users/Downloads/data_archive"));
    }
}
