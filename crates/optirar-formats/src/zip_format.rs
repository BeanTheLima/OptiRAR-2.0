use optirar_core::error::{OptiError, Result};
use optirar_core::traits::{ArchiveReader, ArchiveWriter};
use optirar_core::types::{ArchiveEntry, ArchiveFormat, CompressionLevel};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const CACHE: usize = 16 * 1024;

/// Clonable positional reader. Clones share one OS handle but keep their own cursor and a
/// small read cache, so `Seek` costs no I/O (the zip crate seeks to every local header) and
/// parallel workers can share one parsed central directory.
pub struct SharedFile {
    file: Arc<File>,
    len: u64,
    pos: u64,
    buf: Vec<u8>,
    buf_start: u64,
    buf_len: usize,
}

impl SharedFile {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self { file: Arc::new(file), len, pos: 0, buf: Vec::new(), buf_start: 0, buf_len: 0 })
    }

    fn pread(&self, out: &mut [u8], offset: u64) -> io::Result<usize> {
        #[cfg(windows)]
        {
            std::os::windows::fs::FileExt::seek_read(&*self.file, out, offset)
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::FileExt::read_at(&*self.file, out, offset)
        }
    }
}

impl Clone for SharedFile {
    fn clone(&self) -> Self {
        Self { file: self.file.clone(), len: self.len, pos: self.pos, buf: Vec::new(), buf_start: 0, buf_len: 0 }
    }
}

impl Read for SharedFile {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.pos >= self.len {
            return Ok(0);
        }
        let in_cache = self.pos >= self.buf_start && self.pos < self.buf_start + self.buf_len as u64;
        if !in_cache {
            if out.len() >= CACHE {
                // Large (data) reads bypass the cache.
                let n = self.pread(out, self.pos)?;
                self.pos += n as u64;
                return Ok(n);
            }
            self.buf.resize(CACHE, 0);
            let mut tmp = std::mem::take(&mut self.buf);
            let n = self.pread(&mut tmp, self.pos);
            self.buf = tmp;
            self.buf_len = n?;
            self.buf_start = self.pos;
            if self.buf_len == 0 {
                return Ok(0);
            }
        }
        let off = (self.pos - self.buf_start) as usize;
        let n = out.len().min(self.buf_len - off);
        out[..n].copy_from_slice(&self.buf[off..off + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for SharedFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let new = match to {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::End(d) => self.len as i128 + d as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
        };
        if new < 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before start"));
        }
        self.pos = new as u64;
        Ok(self.pos)
    }
}

/// High-throughput ZIP reader with ZIP64 support.
pub struct ZipReader {
    archive: ZipArchive<SharedFile>,
    password: Option<String>,
}

impl ZipReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::open_with_password(path, None)
    }

    pub fn open_with_password<P: AsRef<Path>>(path: P, password: Option<&str>) -> Result<Self> {
        let archive = ZipArchive::new(SharedFile::open(path)?)
            .map_err(|e| OptiError::MalformedArchive(format!("ZIP open error: {}", e)))?;
        Ok(Self {
            archive,
            password: password.filter(|p| !p.is_empty()).map(String::from),
        })
    }
}

static NAME_CODEPAGE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Code page used for ZIP entry names that are not UTF-8 (0 = Windows OEM code page,
/// e.g. 1258 on Vietnamese Windows, 437 on US). Takes effect on the next archive open.
pub fn set_name_codepage(codepage: u32) {
    NAME_CODEPAGE.store(codepage, std::sync::atomic::Ordering::Relaxed);
}

/// Names that are valid UTF-8 are used as-is (many tools omit the UTF-8 flag);
/// otherwise decode with the configured code page. None = let the zip crate decide (CP437).
fn decode_name(raw: &[u8]) -> Option<String> {
    if let Ok(s) = std::str::from_utf8(raw) {
        return Some(s.to_string());
    }
    decode_codepage(raw, NAME_CODEPAGE.load(std::sync::atomic::Ordering::Relaxed))
}

#[cfg(windows)]
fn decode_codepage(raw: &[u8], codepage: u32) -> Option<String> {
    use windows_sys::Win32::Globalization::{GetOEMCP, MultiByteToWideChar};
    let cp = if codepage == 0 { unsafe { GetOEMCP() } } else { codepage };
    if cp == 437 {
        return None;
    }
    // SAFETY: lengths come from the slices passed in; first call sizes the buffer.
    unsafe {
        let n = MultiByteToWideChar(cp, 0, raw.as_ptr(), raw.len() as i32, std::ptr::null_mut(), 0);
        if n <= 0 {
            return None;
        }
        let mut wide = vec![0u16; n as usize];
        let n = MultiByteToWideChar(cp, 0, raw.as_ptr(), raw.len() as i32, wide.as_mut_ptr(), n);
        (n > 0).then(|| String::from_utf16_lossy(&wide[..n as usize]))
    }
}

#[cfg(not(windows))]
fn decode_codepage(_raw: &[u8], _codepage: u32) -> Option<String> {
    None
}

/// Convert a ZIP (MS-DOS) local date/time to UNIX seconds.
fn zip_time_to_unix(dt: zip::DateTime) -> i64 {
    let (y, m, d) = (dt.year() as i64, dt.month() as i64, dt.day() as i64);
    // days_from_civil (Howard Hinnant)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + dt.hour() as i64 * 3600 + dt.minute() as i64 * 60 + dt.second() as i64
}

impl ArchiveReader for ZipReader {
    fn list_entries(&mut self) -> Result<Vec<ArchiveEntry>> {
        let count = self.archive.len();
        let mut entries = Vec::with_capacity(count);

        for idx in 0..count {
            // Raw access: no decompressor/decryptor setup, works for encrypted entries too.
            let zip_file = self
                .archive
                .by_index_raw(idx)
                .map_err(|e| OptiError::MalformedArchive(format!("ZIP entry read error: {}", e)))?;

            entries.push(ArchiveEntry {
                index: idx,
                path: decode_name(zip_file.name_raw()).unwrap_or_else(|| zip_file.name().to_string()),
                uncompressed_size: zip_file.size(),
                compressed_size: zip_file.compressed_size(),
                is_directory: zip_file.is_dir(),
                is_encrypted: zip_file.encrypted(),
                crc32: Some(zip_file.crc32()),
                hash_hex: None,
                modified_timestamp: zip_file.last_modified().map(zip_time_to_unix),
                windows_attributes: None,
                unix_mode: zip_file.unix_mode(),
            });
        }

        Ok(entries)
    }

    fn extract_entry(&mut self, entry_index: usize, writer: &mut dyn Write) -> Result<()> {
        let encrypted = self
            .archive
            .by_index_raw(entry_index)
            .map(|f| f.encrypted())
            .unwrap_or(false);

        let mut zip_file = if encrypted {
            let pw = self
                .password
                .as_deref()
                .ok_or_else(|| OptiError::MalformedArchive("Password required (encrypted entry)".into()))?;
            self.archive.by_index_decrypt(entry_index, pw.as_bytes())
        } else {
            self.archive.by_index(entry_index)
        }
        .map_err(|e| OptiError::MalformedArchive(format!("ZIP entry read error: {}", e)))?;

        std::io::copy(&mut zip_file, writer)?;
        Ok(())
    }

    fn is_solid(&self) -> bool {
        false
    }

    fn format(&self) -> ArchiveFormat {
        ArchiveFormat::Zip
    }

    fn try_clone(&self) -> Option<Box<dyn ArchiveReader>> {
        Some(Box::new(ZipReader { archive: self.archive.clone(), password: self.password.clone() }))
    }
}

#[cfg(test)]
mod time_tests {
    #[test]
    fn dos_time_converts_to_unix() {
        let dt = zip::DateTime::from_date_and_time(2024, 3, 1, 12, 30, 10).unwrap();
        assert_eq!(super::zip_time_to_unix(dt), 1709296210);
    }

    #[test]
    fn shared_file_reads_match_after_seeks_and_clones() {
        use std::io::{Read, Seek, SeekFrom};
        let data: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let path = std::env::temp_dir().join(format!("optirar_sf_{}.bin", std::process::id()));
        std::fs::write(&path, &data).unwrap();
        let mut f = super::SharedFile::open(&path).unwrap();
        let mut small = [0u8; 10];
        f.seek(SeekFrom::Start(16_380)).unwrap(); // straddles the cache boundary
        f.read_exact(&mut small).unwrap();
        assert_eq!(&small[..], &data[16_380..16_390]);
        let mut g = f.clone();
        let mut rest = Vec::new();
        g.read_to_end(&mut rest).unwrap();
        assert_eq!(&rest[..], &data[16_390..]);
        f.seek(SeekFrom::End(-5)).unwrap();
        f.read_exact(&mut small[..5]).unwrap();
        assert_eq!(&small[..5], &data[data.len() - 5..]);
        let _ = std::fs::remove_file(path);
    }
}

/// ZIP archive creator with automatic ZIP64 support and optional AES-256 encryption.
pub struct CustomZipWriter<W: Write + Seek = File> {
    writer: ZipWriter<W>,
    method: CompressionMethod,
    level: Option<i64>,
    password: Option<String>,
}

fn zip_err(e: zip::result::ZipError) -> OptiError {
    OptiError::Io(io::Error::new(io::ErrorKind::Other, e))
}

fn file_options(method: CompressionMethod, level: Option<i64>, password: Option<&str>) -> zip::write::FileOptions<'_, ()> {
    let o = SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(level)
        .large_file(true); // ZIP64 when a file exceeds 4 GB
    match password {
        Some(pw) => o.with_aes_encryption(zip::AesMode::Aes256, pw),
        None => o,
    }
}

impl CustomZipWriter<File> {
    pub fn create<P: AsRef<Path>>(path: P, level: CompressionLevel) -> Result<Self> {
        Self::create_with_password(path, level, None)
    }

    pub fn create_with_password<P: AsRef<Path>>(path: P, level: CompressionLevel, password: Option<&str>) -> Result<Self> {
        Ok(Self::new(File::create(path)?, level, password))
    }
}

impl<W: Write + Seek> CustomZipWriter<W> {
    pub fn new(inner: W, level: CompressionLevel, password: Option<&str>) -> Self {
        let (method, level) = match level {
            CompressionLevel::Store => (CompressionMethod::Stored, None),
            CompressionLevel::Fastest => (CompressionMethod::Deflated, Some(1)),
            CompressionLevel::Normal => (CompressionMethod::Deflated, Some(6)),
            CompressionLevel::Maximum => (CompressionMethod::Deflated, Some(9)),
        };
        Self {
            writer: ZipWriter::new(inner),
            method,
            level,
            password: password.filter(|p| !p.is_empty()).map(String::from),
        }
    }

    fn add_dir(&mut self, archive_path: &str) -> Result<()> {
        let dir = format!("{}/", archive_path.trim_end_matches('/'));
        self.writer.add_directory(dir, SimpleFileOptions::default()).map_err(zip_err)
    }

    fn add_reader(&mut self, archive_path: &str, reader: &mut dyn Read) -> Result<()> {
        let pw = self.password.clone();
        let opts = file_options(self.method, self.level, pw.as_deref());
        self.writer.start_file(archive_path, opts).map_err(zip_err)?;
        io::copy(reader, &mut self.writer)?;
        Ok(())
    }

    /// Add a file or (recursively) a folder from disk.
    pub fn add_path(&mut self, archive_path: &str, source: &Path) -> Result<()> {
        if source.is_dir() {
            self.add_dir(archive_path)?;
            for entry in std::fs::read_dir(source)? {
                let entry = entry?;
                let child = format!("{}/{}", archive_path.trim_end_matches('/'), entry.file_name().to_string_lossy());
                self.add_path(&child, &entry.path())?;
            }
            Ok(())
        } else {
            self.add_reader(archive_path, &mut File::open(source)?)
        }
    }

    pub fn finish_inner(self) -> Result<W> {
        self.writer.finish().map_err(zip_err)
    }
}

impl ArchiveWriter for CustomZipWriter<File> {
    fn add_file(&mut self, archive_path: &str, source_path: &Path) -> Result<()> {
        if source_path.is_dir() {
            return self.add_dir(archive_path);
        }
        self.add_reader(archive_path, &mut File::open(source_path)?)
    }

    fn add_stream(&mut self, archive_path: &str, reader: &mut dyn Read, _size: u64) -> Result<()> {
        self.add_reader(archive_path, reader)
    }

    fn finish(self: Box<Self>) -> Result<()> {
        self.finish_inner().map(|_| ())
    }
}

/// Content to put into an existing ZIP.
pub enum ZipAddSource {
    Path(std::path::PathBuf),
    Bytes(Vec<u8>),
}

/// Rewrite a ZIP in place: drop entries in `remove` (by index) and any entry replaced by
/// `add` (same name), copy the rest raw (no recompression), then append `add`.
pub fn modify_zip(
    path: &Path,
    remove: &std::collections::HashSet<usize>,
    add: &[(String, ZipAddSource)],
    level: CompressionLevel,
    password: Option<&str>,
) -> Result<()> {
    let tmp = path.with_extension("optirar-tmp");
    let result = (|| -> Result<()> {
        let mut archive = ZipArchive::new(SharedFile::open(path)?)
            .map_err(|e| OptiError::MalformedArchive(format!("ZIP open error: {}", e)))?;
        let replaced: std::collections::HashSet<&str> = add.iter().map(|(n, _)| n.trim_end_matches('/')).collect();
        let mut w = CustomZipWriter::new(File::create(&tmp)?, level, password);
        for i in 0..archive.len() {
            if remove.contains(&i) {
                continue;
            }
            let file = archive.by_index_raw(i).map_err(zip_err)?;
            if replaced.contains(file.name().trim_end_matches('/')) {
                continue;
            }
            w.writer.raw_copy_file(file).map_err(zip_err)?;
        }
        for (name, src) in add {
            match src {
                ZipAddSource::Path(p) => w.add_path(name, p)?,
                ZipAddSource::Bytes(b) => w.add_reader(name, &mut b.as_slice())?,
            }
        }
        w.finish_inner()?;
        Ok(())
    })();
    // The source handle is closed here, so Windows allows replacing the file.
    match result {
        Ok(()) => std::fs::rename(&tmp, path).map_err(OptiError::from),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod write_tests {
    use super::*;
    use optirar_core::traits::ArchiveReader;

    fn read_all(path: &Path, pw: Option<&str>) -> Vec<(String, Vec<u8>)> {
        let mut r = ZipReader::open_with_password(path, pw).unwrap();
        let entries = r.list_entries().unwrap();
        entries
            .iter()
            .filter(|e| !e.is_directory)
            .map(|e| {
                let mut buf = Vec::new();
                r.extract_entry(e.index, &mut buf).unwrap();
                (e.path.clone(), buf)
            })
            .collect()
    }

    #[test]
    fn aes_zip_roundtrip_and_modify() {
        let dir = std::env::temp_dir().join(format!("optirar_zw_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("t.zip");
        let mut w = CustomZipWriter::create_with_password(&zip, CompressionLevel::Normal, Some("bí mật")).unwrap();
        w.add_stream("a.txt", &mut &b"alpha"[..], 5).unwrap();
        w.add_stream("dir/b.txt", &mut &b"beta"[..], 4).unwrap();
        Box::new(w).finish().unwrap();

        assert!(ZipReader::open(&zip).unwrap().extract_entry(0, &mut Vec::new()).is_err());
        assert_eq!(read_all(&zip, Some("bí mật"))[0].1, b"alpha");

        // delete a.txt, replace dir/b.txt, add c.txt (no password => plain entries)
        let remove: std::collections::HashSet<usize> = [0].into_iter().collect();
        let add = vec![
            ("dir/b.txt".to_string(), ZipAddSource::Bytes(b"BETA2".to_vec())),
            ("c.txt".to_string(), ZipAddSource::Bytes(b"gamma".to_vec())),
        ];
        modify_zip(&zip, &remove, &add, CompressionLevel::Normal, None).unwrap();
        let all = read_all(&zip, Some("bí mật"));
        let names: Vec<_> = all.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["dir/b.txt", "c.txt"]);
        assert_eq!(all[0].1, b"BETA2");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn non_utf8_names_use_codepage() {
        assert_eq!(decode_name("tệp.txt".as_bytes()).as_deref(), Some("tệp.txt"));
        #[cfg(windows)]
        assert_eq!(decode_codepage(&[0x54, 0xEA, 0x6E], 1258).as_deref(), Some("Tên"));
    }
}
