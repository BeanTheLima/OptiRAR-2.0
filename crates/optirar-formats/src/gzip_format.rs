use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use optirar_core::error::{OptiError, Result};
use optirar_core::traits::{ArchiveReader, ArchiveWriter};
use optirar_core::types::{ArchiveEntry, ArchiveFormat, CompressionLevel};
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// GZIP archive reader supporting both standalone .gz and .tar.gz archives.
pub struct GzipReader {
    file_path: PathBuf,
    is_tar_gz: bool,
    tar_entries: Vec<ArchiveEntry>,
}

impl GzipReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let name_lower = path_buf.to_string_lossy().to_ascii_lowercase();
        let is_tar_gz = name_lower.ends_with(".tar.gz") || name_lower.ends_with(".tgz");

        let mut tar_entries = Vec::new();

        if is_tar_gz {
            let file = File::open(&path_buf)?;
            let gz_decoder = GzDecoder::new(BufReader::new(file));
            let mut archive = tar::Archive::new(gz_decoder);

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
            // Standalone .gz file: strip .gz from filename
            let stem = path_buf
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "extracted_file".to_string());
            let file_size = path_buf.metadata()?.len();

            tar_entries.push(ArchiveEntry {
                index: 0,
                path: stem,
                uncompressed_size: file_size * 2, // heuristic until decompressed
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
            is_tar_gz,
            tar_entries,
        })
    }
}

impl ArchiveReader for GzipReader {
    fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>> {
        Ok(self.tar_entries.clone())
    }

    fn extract_entry(&mut self, entry_index: usize, writer: &mut dyn Write) -> Result<()> {
        let file = File::open(&self.file_path)?;
        let mut gz_decoder = GzDecoder::new(BufReader::new(file));

        if self.is_tar_gz {
            let mut archive = tar::Archive::new(gz_decoder);
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
                let n = gz_decoder.read(&mut buffer)?;
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
        ArchiveFormat::Gzip
    }
}

/// GZIP archive writer.
pub struct GzipWriterWrapper {
    encoder: GzEncoder<File>,
}

impl GzipWriterWrapper {
    pub fn create<P: AsRef<Path>>(path: P, level: CompressionLevel) -> Result<Self> {
        let file = File::create(path)?;
        let comp = match level {
            CompressionLevel::Store => Compression::none(),
            CompressionLevel::Fastest => Compression::fast(),
            CompressionLevel::Normal => Compression::default(),
            CompressionLevel::Maximum => Compression::best(),
        };
        let encoder = GzEncoder::new(file, comp);
        Ok(Self { encoder })
    }
}

impl ArchiveWriter for GzipWriterWrapper {
    fn add_file(&mut self, _archive_path: &str, source_path: &Path) -> Result<()> {
        let mut file = File::open(source_path)?;
        let mut buffer = [0u8; 128 * 1024];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            self.encoder.write_all(&buffer[..n])?;
        }
        Ok(())
    }

    fn add_stream(&mut self, _archive_path: &str, reader: &mut dyn Read, _size: u64) -> Result<()> {
        let mut buffer = [0u8; 128 * 1024];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            self.encoder.write_all(&buffer[..n])?;
        }
        Ok(())
    }

    fn finish(self: Box<Self>) -> Result<()> {
        self.encoder.finish()?;
        Ok(())
    }
}
