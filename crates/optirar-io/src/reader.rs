use optirar_core::error::Result;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Fast sequential file reader optimized for large archive streaming.
/// Employs OS-level sequential scan hints (`FILE_FLAG_SEQUENTIAL_SCAN` on Windows)
/// to maximize storage controller read-ahead performance.
pub struct FastFileReader {
    inner: BufReader<File>,
    file_size: u64,
}

impl FastFileReader {
    /// Open a file with the specified buffer capacity and sequential scan flags.
    pub fn open<P: AsRef<Path>>(path: P, buffer_size: usize) -> Result<Self> {
        let path_ref = path.as_ref();

        #[cfg(windows)]
        let file = {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x08000000;
            std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN)
                .open(path_ref)?
        };

        #[cfg(not(windows))]
        let file = std::fs::File::open(path_ref)?;

        let file_size = file.metadata()?.len();
        let reader = BufReader::with_capacity(buffer_size.max(64 * 1024), file);

        Ok(Self {
            inner: reader,
            file_size,
        })
    }

    /// Total file size in bytes.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Consume the reader and return the inner `File`.
    pub fn into_inner(self) -> File {
        self.inner.into_inner()
    }

    /// Access reference to underlying File.
    pub fn get_ref(&self) -> &File {
        self.inner.get_ref()
    }
}

impl Read for FastFileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Seek for FastFileReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}
