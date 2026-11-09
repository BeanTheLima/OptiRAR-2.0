use optirar_core::error::Result;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

/// High-throughput file writer that pre-allocates disk clusters to eliminate NTFS fragmentation.
pub struct PreallocatedFileWriter {
    writer: BufWriter<File>,
    bytes_written: u64,
    expected_size: u64,
}

impl PreallocatedFileWriter {
    /// Create a new file, pre-allocating the target disk size if known.
    pub fn create<P: AsRef<Path>>(
        path: P,
        expected_size: u64,
        buffer_size: usize,
    ) -> Result<Self> {
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path_ref)?;

        // Pre-allocate disk space if expected size is known and > 0.
        // This prompts NTFS to assign contiguous clusters, minimizing filesystem fragmentation.
        if expected_size > 0 {
            let _ = file.set_len(expected_size);
        }

        let writer = BufWriter::with_capacity(buffer_size.max(64 * 1024), file);

        Ok(Self {
            writer,
            bytes_written: 0,
            expected_size,
        })
    }

    /// Number of bytes successfully written so far.
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Expected total size in bytes.
    pub fn expected_size(&self) -> u64 {
        self.expected_size
    }

    /// Finalize writes, truncate to exact written bytes (if different from pre-allocated length), and flush.
    pub fn finalize(mut self) -> Result<u64> {
        self.writer.flush()?;
        let file = self.writer.get_mut();
        // If written bytes were less than expected, adjust file length accurately.
        if self.bytes_written != self.expected_size {
            let _ = file.set_len(self.bytes_written);
        }
        Ok(self.bytes_written)
    }
}

impl Write for PreallocatedFileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.writer.write(buf)?;
        self.bytes_written += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
