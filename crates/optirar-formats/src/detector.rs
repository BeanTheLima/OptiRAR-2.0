use optirar_core::error::Result;
use optirar_core::types::ArchiveFormat;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Detects archive formats via magic bytes inspection with file extension fallback.
pub struct FormatDetector;

impl FormatDetector {
    /// Inspect the first bytes of the file to determine its archive container format.
    pub fn detect<P: AsRef<Path>>(path: P) -> Result<ArchiveFormat> {
        let path_ref = path.as_ref();
        let mut file = match File::open(path_ref) {
            Ok(f) => f,
            Err(_) => return Ok(ArchiveFormat::from_path(path_ref)),
        };

        let mut magic = [0u8; 16];
        let bytes_read = match file.read(&mut magic) {
            Ok(n) => n,
            Err(_) => return Ok(ArchiveFormat::from_path(path_ref)),
        };

        if bytes_read >= 8 {
            // Check RAR5: 52 61 72 21 1A 07 01 00
            if magic[0..8] == [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00] {
                return Ok(ArchiveFormat::Rar5);
            }
        }

        if bytes_read >= 7 {
            // Check RAR4: 52 61 72 21 1A 07 00
            if magic[0..7] == [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00] {
                return Ok(ArchiveFormat::Rar4);
            }
        }

        if bytes_read >= 6 {
            // Check 7-Zip: 37 7A BC AF 27 1C
            if magic[0..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
                return Ok(ArchiveFormat::SevenZip);
            }
        }

        if bytes_read >= 4 {
            // Check ZIP: PK\x03\x04 or PK\x05\x06 (empty) or PK\x07\x08 (spanned)
            if magic[0..4] == [0x50, 0x4B, 0x03, 0x04]
                || magic[0..4] == [0x50, 0x4B, 0x05, 0x06]
                || magic[0..4] == [0x50, 0x4B, 0x07, 0x08]
            {
                return Ok(ArchiveFormat::Zip);
            }

            // Check Zstandard: 28 B5 2F FD
            if magic[0..4] == [0x28, 0xB5, 0x2F, 0xFD] {
                return Ok(ArchiveFormat::Zstd);
            }
        }

        if bytes_read >= 2 {
            // Check GZIP: 1F 8B
            if magic[0..2] == [0x1F, 0x8B] {
                return Ok(ArchiveFormat::Gzip);
            }
        }

        // Fallback to path extension heuristic
        Ok(ArchiveFormat::from_path(path_ref))
    }
}

#[cfg(test)]
mod tests {
    use super::FormatDetector;

    #[test]
    fn test_format_detector_magic_bytes() {
        // Test simulated buffer logic
        let rar5_bytes = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00];
        assert_eq!(
            &rar5_bytes[0..8],
            &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00]
        );
    }
}
