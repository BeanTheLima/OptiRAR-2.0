use optirar_core::error::{OptiError, Result};
use optirar_core::traits::{ArchiveReader, ArchiveWriter};
use optirar_core::types::{ArchiveEntry, ArchiveFormat};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tar::{Archive, Builder};

/// TAR archive reader.
pub struct TarReader {
    file_path: std::path::PathBuf,
    entries: Vec<ArchiveEntry>,
}

impl TarReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let file = File::open(&path_buf)?;
        let mut archive = Archive::new(file);

        let mut entries = Vec::new();
        let mut idx = 0;

        for entry_res in archive.entries()? {
            let entry = entry_res?;
            let header = entry.header();
            let path_str = entry.path()?.to_string_lossy().to_string();
            let size = entry.size();
            let is_dir = header.entry_type().is_dir();
            let mtime = header.mtime().ok().map(|t| t as i64);
            let mode = header.mode().ok();

            entries.push(ArchiveEntry {
                index: idx,
                path: path_str,
                uncompressed_size: size,
                compressed_size: size,
                is_directory: is_dir,
                is_encrypted: false,
                crc32: None,
                hash_hex: None,
                modified_timestamp: mtime,
                windows_attributes: None,
                unix_mode: mode,
            });
            idx += 1;
        }

        Ok(Self {
            file_path: path_buf,
            entries,
        })
    }
}

impl ArchiveReader for TarReader {
    fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>> {
        Ok(self.entries.clone())
    }

    fn extract_entry(&mut self, entry_index: usize, writer: &mut dyn Write) -> Result<()> {
        let file = File::open(&self.file_path)?;
        let mut archive = Archive::new(file);

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
    }

    fn is_solid(&self) -> bool {
        true
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Tar
    }
}

/// TAR archive creator.
pub struct TarWriterWrapper {
    builder: Builder<File>,
}

impl TarWriterWrapper {
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::create(path)?;
        let builder = Builder::new(file);
        Ok(Self { builder })
    }
}

impl ArchiveWriter for TarWriterWrapper {
    fn add_file(&mut self, archive_path: &str, source_path: &Path) -> Result<()> {
        if source_path.is_dir() {
            self.builder
                .append_dir(archive_path, source_path)
                .map_err(OptiError::Io)?;
        } else {
            let mut file = File::open(source_path)?;
            self.builder
                .append_file(archive_path, &mut file)
                .map_err(OptiError::Io)?;
        }
        Ok(())
    }

    fn add_stream(&mut self, archive_path: &str, reader: &mut dyn Read, size: u64) -> Result<()> {
        let mut header = tar::Header::new_gnu();
        header.set_size(size);
        header.set_mode(0o644);
        header.set_cksum();
        self.builder
            .append_data(&mut header, archive_path, reader)
            .map_err(OptiError::Io)?;
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.builder.finish()?;
        Ok(())
    }
}
