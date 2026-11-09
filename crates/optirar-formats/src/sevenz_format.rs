use optirar_core::error::{OptiError, Result};
use optirar_core::traits::ArchiveReader;
use optirar_core::types::{ArchiveEntry, ArchiveFormat};
use sevenz_rust::{Password, SevenZReader};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Pure-Rust 7-Zip reader supporting LZMA, LZMA2, and AES-256 decryption.
pub struct SevenZipReader {
    file_path: PathBuf,
    password: Password,
    entries: Vec<ArchiveEntry>,
    is_solid: bool,
}

impl SevenZipReader {
    pub fn open<P: AsRef<Path>>(path: P, password: Option<&str>) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let pass = match password {
            Some(p) => Password::from(p),
            None => Password::empty(),
        };

        let sz_reader = SevenZReader::open(&path_buf, pass.clone())
            .map_err(|e| OptiError::MalformedArchive(format!("7z open error: {:?}", e)))?;

        let mut entries = Vec::new();
        let mut is_solid = false;

        let archive = sz_reader.archive();
        if !archive.folders.is_empty() {
            is_solid = archive.folders.iter().any(|f| f.num_unpack_sub_streams > 1);
        }

        for (idx, entry) in archive.files.iter().enumerate() {
            let path_str = entry.name.clone();
            let uncompressed_size = entry.size;
            let compressed_size = entry.compressed_size;
            let is_dir = entry.is_directory;
            let crc = if entry.has_crc {
                Some(entry.crc as u32)
            } else {
                None
            };
            let win_attr = if entry.has_windows_attributes {
                Some(entry.windows_attributes)
            } else {
                None
            };

            entries.push(ArchiveEntry {
                index: idx,
                path: path_str,
                uncompressed_size,
                compressed_size,
                is_directory: is_dir,
                is_encrypted: false,
                crc32: crc,
                hash_hex: None,
                modified_timestamp: None,
                windows_attributes: win_attr,
                unix_mode: None,
            });
        }

        Ok(Self {
            file_path: path_buf,
            password: pass,
            entries,
            is_solid,
        })
    }
}

impl ArchiveReader for SevenZipReader {
    fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>> {
        Ok(self.entries.clone())
    }

    fn extract_entry(&mut self, entry_index: usize, writer: &mut dyn Write) -> Result<()> {
        let mut sz_reader = SevenZReader::open(&self.file_path, self.password.clone())
            .map_err(|e| OptiError::MalformedArchive(format!("7z open error: {:?}", e)))?;

        let mut current_idx = 0;
        let mut found = false;

        sz_reader
            .for_each_entries(|_entry, reader| {
                if current_idx == entry_index {
                    found = true;
                    let mut buffer = [0u8; 128 * 1024];
                    loop {
                        let n = std::io::Read::read(reader, &mut buffer).map_err(|e| {
                            sevenz_rust::Error::other(format!("Read error: {}", e))
                        })?;
                        if n == 0 {
                            break;
                        }
                        writer.write_all(&buffer[..n]).map_err(|e| {
                            sevenz_rust::Error::other(format!("Write error: {}", e))
                        })?;
                    }
                    Ok(false)
                } else {
                    current_idx += 1;
                    Ok(true)
                }
            })
            .map_err(|e| OptiError::MalformedArchive(format!("7z extract error: {:?}", e)))?;

        if found {
            Ok(())
        } else {
            Err(OptiError::EntryNotFound(format!(
                "Entry index {} not found in 7z archive",
                entry_index
            )))
        }
    }

    fn is_solid(&self) -> bool {
        self.is_solid
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::SevenZip
    }
}

/// 7-Zip creator (LZMA2, optional AES-256 with encrypted headers).
pub struct SevenZipWriterWrapper {
    writer: sevenz_rust::SevenZWriter<std::fs::File>,
}

impl SevenZipWriterWrapper {
    pub fn create<P: AsRef<Path>>(
        path: P,
        level: optirar_core::types::CompressionLevel,
        password: Option<&str>,
    ) -> Result<Self> {
        use optirar_core::types::CompressionLevel;
        let mut writer = sevenz_rust::SevenZWriter::create(path.as_ref())
            .map_err(|e| OptiError::Internal(format!("7z create error: {:?}", e)))?;
        let preset = match level {
            CompressionLevel::Store => 0,
            CompressionLevel::Fastest => 1,
            CompressionLevel::Normal => 6,
            CompressionLevel::Maximum => 9,
        };
        let mut methods: Vec<sevenz_rust::SevenZMethodConfiguration> = Vec::new();
        if let Some(pw) = password.filter(|p| !p.is_empty()) {
            methods.push(sevenz_rust::AesEncoderOptions::new(Password::from(pw)).into());
        }
        methods.push(sevenz_rust::lzma::LZMA2Options::with_preset(preset).into());
        writer.set_content_methods(methods);
        Ok(Self { writer })
    }

    fn push(&mut self, entry: sevenz_rust::SevenZArchiveEntry, reader: Option<&mut dyn std::io::Read>) -> Result<()> {
        self.writer
            .push_archive_entry(entry, reader)
            .map_err(|e| OptiError::Internal(format!("7z write error: {:?}", e)))?;
        Ok(())
    }
}

impl optirar_core::traits::ArchiveWriter for SevenZipWriterWrapper {
    fn add_file(&mut self, archive_path: &str, source_path: &Path) -> Result<()> {
        let entry = sevenz_rust::SevenZArchiveEntry::from_path(source_path, archive_path.to_string());
        if source_path.is_dir() {
            return self.push(entry, None);
        }
        let mut file = std::fs::File::open(source_path)?;
        self.push(entry, Some(&mut file))
    }

    fn add_stream(&mut self, archive_path: &str, reader: &mut dyn std::io::Read, _size: u64) -> Result<()> {
        let mut entry = sevenz_rust::SevenZArchiveEntry::new();
        entry.name = archive_path.to_string();
        entry.has_stream = true;
        self.push(entry, Some(reader))
    }

    fn finish(self: Box<Self>) -> Result<()> {
        self.writer.finish()?;
        Ok(())
    }
}

#[cfg(test)]
mod write_tests {
    use super::*;
    use optirar_core::traits::ArchiveWriter;

    #[test]
    fn encrypted_7z_roundtrip() {
        let path = std::env::temp_dir().join(format!("optirar_7z_{}.7z", std::process::id()));
        let mut w = SevenZipWriterWrapper::create(&path, optirar_core::types::CompressionLevel::Fastest, Some("mk")).unwrap();
        w.add_stream("thư mục/a.txt", &mut &b"xin chao"[..], 8).unwrap();
        Box::new(w).finish().unwrap();

        if let Ok(mut r) = SevenZipReader::open(&path, None) {
            let e = r.list_entries().unwrap();
            assert!(r.extract_entry(e[0].index, &mut Vec::new()).is_err(), "content must be encrypted");
        }
        let mut r = SevenZipReader::open(&path, Some("mk")).unwrap();
        let entries = r.list_entries().unwrap();
        assert_eq!(entries[0].path.replace(char::from(92), "/"), "thư mục/a.txt");
        let mut out = Vec::new();
        r.extract_entry(entries[0].index, &mut out).unwrap();
        assert_eq!(out, b"xin chao");
        let _ = std::fs::remove_file(path);
    }
}
