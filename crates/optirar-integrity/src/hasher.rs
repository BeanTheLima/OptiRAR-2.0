use optirar_core::{CancellationToken, OptiError, Result};
use sha2::Digest;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FileHashes {
    pub file_path: PathBuf,
    pub file_size: u64,
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
    pub sha512: String,
    pub blake3: String,
    pub elapsed_seconds: f64,
    pub throughput_mb_s: f64,
}

pub struct MultiHasher;

impl MultiHasher {
    /// Compute all standard checksums and cryptographic hashes in a single streaming pass.
    pub fn compute_all<P: AsRef<Path>>(
        path: P,
        cancel: Option<&CancellationToken>,
    ) -> Result<FileHashes> {
        let path = path.as_ref();
        let mut file = File::open(path)
            .map_err(|e| OptiError::Io(e))?;

        let metadata = file.metadata()
            .map_err(|e| OptiError::Io(e))?;
        let file_size = metadata.len();

        let mut crc_hasher = crc32fast::Hasher::new();
        let mut md5_hasher = md5::Md5::new();
        let mut sha1_hasher = sha1::Sha1::new();
        let mut sha256_hasher = sha2::Sha256::new();
        let mut sha512_hasher = sha2::Sha512::new();
        let mut blake3_hasher = blake3::Hasher::new();

        let mut buffer = vec![0u8; 128 * 1024]; // 128 KB buffer
        let start = Instant::now();
        let mut bytes_read_total = 0u64;

        loop {
            if let Some(token) = cancel {
                if token.is_cancelled() {
                    return Err(OptiError::OperationCancelled);
                }
            }

            let n = file.read(&mut buffer)
                .map_err(|e| OptiError::Io(e))?;
            if n == 0 {
                break;
            }

            let chunk = &buffer[..n];
            crc_hasher.update(chunk);
            md5_hasher.update(chunk);
            sha1_hasher.update(chunk);
            sha256_hasher.update(chunk);
            sha512_hasher.update(chunk);
            blake3_hasher.update(chunk);

            bytes_read_total += n as u64;
        }

        let elapsed = start.elapsed().as_secs_f64().max(0.0001);
        let throughput = (bytes_read_total as f64 / (1024.0 * 1024.0)) / elapsed;

        let crc32_val = crc_hasher.finalize();
        let md5_val = md5_hasher.finalize();
        let sha1_val = sha1_hasher.finalize();
        let sha256_val = sha256_hasher.finalize();
        let sha512_val = sha512_hasher.finalize();
        let blake3_val = blake3_hasher.finalize();

        Ok(FileHashes {
            file_path: path.to_path_buf(),
            file_size,
            crc32: format!("{:08X}", crc32_val),
            md5: format!("{:x}", md5_val),
            sha1: format!("{:x}", sha1_val),
            sha256: format!("{:x}", sha256_val),
            sha512: format!("{:x}", sha512_val),
            blake3: blake3_val.to_hex().to_string(),
            elapsed_seconds: elapsed,
            throughput_mb_s: throughput,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_compute_all_hashes() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("optirar_hash_test.txt");
        let mut f = File::create(&test_file).unwrap();
        f.write_all(b"Hello OptiRAR 2.0! Fast, Modern, and Secure Archive Manager.").unwrap();
        drop(f);

        let hashes = MultiHasher::compute_all(&test_file, None).unwrap();
        assert_eq!(hashes.crc32.len(), 8);
        assert_eq!(hashes.md5.len(), 32);
        assert_eq!(hashes.sha1.len(), 40);
        assert_eq!(hashes.sha256.len(), 64);
        assert_eq!(hashes.sha512.len(), 128);
        assert_eq!(hashes.blake3.len(), 64);

        let _ = std::fs::remove_file(&test_file);
    }
}
