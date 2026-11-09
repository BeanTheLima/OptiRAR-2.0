use optirar_core::error::{OptiError, Result};
use optirar_formats::open_reader;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreviewContent {
    pub entry_path: String,
    pub original_size: u64,
    pub is_text: bool,
    pub text_preview: Option<String>,
    pub is_truncated: bool,
    pub preview_bytes_len: usize,
}

struct BoundedWriter {
    buffer: Vec<u8>,
    max_bytes: usize,
    total_written: usize,
}

impl BoundedWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(max_bytes.min(64 * 1024)),
            max_bytes,
            total_written: 0,
        }
    }

    fn is_truncated(&self) -> bool {
        self.total_written > self.max_bytes
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len();
        self.total_written += len;

        let remaining = self.max_bytes.saturating_sub(self.buffer.len());
        if remaining > 0 {
            let to_copy = remaining.min(len);
            self.buffer.extend_from_slice(&buf[..to_copy]);
        }
        // Always report full len accepted so decompressors don't abort with broken pipe
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct ArchivePreviewer;

impl ArchivePreviewer {
    /// Raw bytes of entry `index` (at most `max_bytes`) and whether the entry was longer.
    pub fn read_entry<P: AsRef<Path>>(
        archive_path: P,
        index: usize,
        password: Option<&str>,
        max_bytes: usize,
    ) -> Result<(Vec<u8>, bool)> {
        let mut reader = open_reader(archive_path.as_ref(), password)?;
        let mut writer = BoundedWriter::new(max_bytes);
        reader.extract_entry(index, &mut writer)?;
        let truncated = writer.is_truncated();
        Ok((writer.buffer, truncated))
    }

    /// Preview an entry inside an archive with bounded memory usage (max 512 KB by default).
    pub fn preview_entry<P: AsRef<Path>>(
        archive_path: P,
        entry_path_to_find: &str,
        password: Option<&str>,
        max_bytes: usize,
    ) -> Result<PreviewContent> {
        let mut reader = open_reader(archive_path.as_ref(), password)?;
        let entries = reader.list_entries()?;

        // Find entry by path
        let normalized_target = entry_path_to_find.replace('\\', "/").trim_start_matches('/').to_string();
        let target_idx = entries.iter().position(|e| {
            let norm = e.path.replace('\\', "/").trim_start_matches('/').to_string();
            norm == normalized_target
        }).ok_or_else(|| OptiError::EntryNotFound(entry_path_to_find.to_string()))?;

        let entry = &entries[target_idx];
        if entry.is_directory {
            return Ok(PreviewContent {
                entry_path: entry.path.clone(),
                original_size: 0,
                is_text: true,
                text_preview: Some(format!("[Directory: {}]", entry.path)),
                is_truncated: false,
                preview_bytes_len: 0,
            });
        }

        let mut writer = BoundedWriter::new(max_bytes);
        reader.extract_entry(target_idx, &mut writer)?;

        let is_truncated = writer.is_truncated();
        let bytes = writer.buffer;
        let preview_len = bytes.len();

        // Detect text vs binary: inspect first 8KB for null bytes
        let inspect_slice = &bytes[..bytes.len().min(8192)];
        let has_null_byte = inspect_slice.contains(&0);

        if !has_null_byte {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                return Ok(PreviewContent {
                    entry_path: entry.path.clone(),
                    original_size: entry.uncompressed_size,
                    is_text: true,
                    text_preview: Some(text.to_string()),
                    is_truncated,
                    preview_bytes_len: preview_len,
                });
            }
        }

        // Binary content
        Ok(PreviewContent {
            entry_path: entry.path.clone(),
            original_size: entry.uncompressed_size,
            is_text: false,
            text_preview: None,
            is_truncated,
            preview_bytes_len: preview_len,
        })
    }
}
