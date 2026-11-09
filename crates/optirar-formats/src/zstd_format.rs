use optirar_core::error::{OptiError, Result};
use optirar_core::traits::{ArchiveReader, ArchiveWriter};
use optirar_core::types::{ArchiveEntry, ArchiveFormat, CompressionLevel};
use ruzstd::decoding::StreamingDecoder;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Pure-Rust Zstandard reader supporting both standalone .zst and .tar.zst archives.
pub struct ZstdReader {
    file_path: PathBuf,
    is_tar_zst: bool,
    tar_entries: Vec<ArchiveEntry>,
}

impl ZstdReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let name_lower = path_buf.to_string_lossy().to_ascii_lowercase();
        let is_tar_zst = name_lower.ends_with(".tar.zst") || name_lower.ends_with(".tzst");

        let mut tar_entries = Vec::new();

        if is_tar_zst {
            let file = File::open(&path_buf)?;
            let zstd_decoder = StreamingDecoder::new(BufReader::new(file))
                .map_err(|e| OptiError::MalformedArchive(format!("ZSTD init error: {:?}", e)))?;
            let mut archive = tar::Archive::new(zstd_decoder);

            let mut idx = 0;
            for entry_res in archive.entries()? {
                let entry = entry_res?;
                let header = entry.header();
                let path_str = entry.path()?.to_string_lossy().to_string();
                let size = entry.size();
                let is_dir = header.entry_type().is_dir();
                let mtime = header.mtime().ok().map(|t| t as i64);

                tar_entries.push(ArchiveEntry {
                    index: idx,
                    path: path_str,
                    uncompressed_size: size,
                    compressed_size: 0,
                    is_directory: is_dir,
                    is_encrypted: false,
                    crc32: None,
                    hash_hex: None,
                    modified_timestamp: mtime,
                    windows_attributes: None,
                    unix_mode: header.mode().ok(),
                });
                idx += 1;
            }
        } else {
            let stem = path_buf
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "extracted_file".to_string());
            let file_size = path_buf.metadata()?.len();

            tar_entries.push(ArchiveEntry {
                index: 0,
                path: stem,
                uncompressed_size: file_size * 2,
                compressed_size: file_size,
                is_directory: false,
                is_encrypted: false,
                crc32: None,
                hash_hex: None,
                modified_timestamp: None,
                windows_attributes: None,
                unix_mode: None,
            });
        }

        Ok(Self {
            file_path: path_buf,
            is_tar_zst,
            tar_entries,
        })
    }
}

impl ArchiveReader for ZstdReader {
    fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>> {
        Ok(self.tar_entries.clone())
    }

    fn extract_entry(&mut self, entry_index: usize, writer: &mut dyn Write) -> Result<()> {
        let file = File::open(&self.file_path)?;
        let mut zstd_decoder = StreamingDecoder::new(BufReader::new(file))
            .map_err(|e| OptiError::MalformedArchive(format!("ZSTD init error: {:?}", e)))?;

        if self.is_tar_zst {
            let mut archive = tar::Archive::new(zstd_decoder);
            let mut current_idx = 0;
            for entry_res in archive.entries()? {
                let mut entry = entry_res?;
                if current_idx == entry_index {
                    let mut buffer = [0u8; 128 * 1024];
                    loop {
                        let n = entry.read(&mut buffer)?;
                        if n == 0 {
                            break;
                        }
                        writer.write_all(&buffer[..n])?;
                    }
                    return Ok(());
                }
                current_idx += 1;
            }
            Err(OptiError::EntryNotFound(format!(
                "Entry index {} not found",
                entry_index
            )))
        } else {
            let mut buffer = [0u8; 128 * 1024];
            loop {
                let n = zstd_decoder.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                writer.write_all(&buffer[..n])?;
            }
            Ok(())
        }
    }

    fn is_solid(&self) -> bool {
        true
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Zstd
    }
}

/// Zstandard writer.
pub struct ZstdWriterWrapper;

impl ZstdWriterWrapper {
    pub fn create<P: AsRef<Path>>(_path: P, _level: CompressionLevel) -> Result<Self> {
        Err(OptiError::CreationNotSupported(ArchiveFormat::Zstd))
    }
}

impl ArchiveWriter for ZstdWriterWrapper {
    fn add_file(&mut self, _archive_path: &str, _source_path: &Path) -> Result<()> {
        Err(OptiError::CreationNotSupported(ArchiveFormat::Zstd))
    }

    fn add_stream(&mut self, _archive_path: &str, _reader: &mut dyn Read, _size: u64) -> Result<()> {
        Err(OptiError::CreationNotSupported(ArchiveFormat::Zstd))
    }

    fn finish(self: Box<Self>) -> Result<()> {
        Err(OptiError::CreationNotSupported(ArchiveFormat::Zstd))
    }
}
