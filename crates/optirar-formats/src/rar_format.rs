use optirar_core::error::{OptiError, Result};
use optirar_core::traits::ArchiveReader;
use optirar_core::types::{ArchiveEntry, ArchiveFormat};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use unrar_rs::RarArchive;

/// Pure-Rust RAR reader supporting RAR4 and RAR5 formats without C FFI or proprietary code.
pub struct RarReader {
    file_path: PathBuf,
    entries: Vec<ArchiveEntry>,
    is_solid: bool,
    format: ArchiveFormat,
}

impl RarReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::open(&path_buf)?;

        let archive = RarArchive::open(file)
            .map_err(|e| OptiError::MalformedArchive(format!("RAR open error: {:?}", e)))?;

        let mut entries = Vec::new();
        let mut is_solid = false;

        for (idx, member) in archive.entries().enumerate() {
            let path_str = member.name.clone();
            let uncompressed_size = member.unpacked_size.unwrap_or(0);
            let compressed_size = member.compressed_size;
            let is_dir = member.is_directory;
            let is_encrypted = member.is_encrypted;
            let crc = member.crc32;
            let hash_hex = member.hash.map(|h| match h {
                unrar_rs::FileHash::Blake2sp(bytes) => {
                    bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>()
                }
            });

            if member.compression.solid {
                is_solid = true;
            }

            let mtime = member
                .mtime
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);

            entries.push(ArchiveEntry {
                index: idx,
                path: path_str,
                uncompressed_size,
                compressed_size,
                is_directory: is_dir,
                is_encrypted,
                crc32: crc,
                hash_hex,
                modified_timestamp: mtime,
                windows_attributes: Some(member.attributes.0 as u32),
                unix_mode: None,
            });
        }

        // Determine format from magic bytes (RAR4 vs RAR5)
        let format = crate::detector::FormatDetector::detect(&path_buf)
            .unwrap_or(ArchiveFormat::Rar5);

        Ok(Self {
            file_path: path_buf,
            entries,
            is_solid,
            format,
        })
    }
}

impl ArchiveReader for RarReader {
    fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>> {
        Ok(self.entries.clone())
    }

    fn extract_entry(&mut self, entry_index: usize, writer: &mut dyn Write) -> Result<()> {
        let file = File::open(&self.file_path)?;
        let mut archive = RarArchive::open(file)
            .map_err(|e| OptiError::MalformedArchive(format!("RAR open error: {:?}", e)))?;

        let entry = archive
            .by_index(entry_index)
            .map_err(|e| OptiError::MalformedArchive(format!("RAR seek error: {:?}", e)))?;

        entry
            .copy_to(writer)
            .map_err(|e| OptiError::MalformedArchive(format!("RAR extraction error: {:?}", e)))?;

        Ok(())
    }

    fn is_solid(&self) -> bool {
        self.is_solid
    }

    fn format(&self) -> ArchiveFormat {
        self.format
    }
}
