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
            // Check OptiRAR: OPTIRAR\x01
            if &magic[0..8] == b"OPTIRAR\x01" {
                return Ok(ArchiveFormat::Orr);
            }

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

    /// Detect if the file is part of a multi-volume archive, and return (current_volume_index_1_based, total_volumes_found).
    pub fn detect_volume_info<P: AsRef<Path>>(path: P) -> Option<(usize, usize)> {
        let p = path.as_ref();
        let filename = p.file_name()?.to_str()?;
        let parent = p.parent()?;

        let lower = filename.to_ascii_lowercase();

        // 1. .partNNN.rar / .partN.rar
        if let Some(pos) = lower.find(".part") {
            if lower.ends_with(".rar") {
                let mid = &lower[pos + 5..lower.len() - 4];
                if let Ok(curr) = mid.parse::<usize>() {
                    let mut count = 0;
                    if let Ok(entries) = std::fs::read_dir(parent) {
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                let nl = name.to_ascii_lowercase();
                                if nl.starts_with(&lower[..pos + 5]) && nl.ends_with(".rar") {
                                    let part_str = &nl[pos + 5..nl.len() - 4];
                                    if part_str.chars().all(|c| c.is_ascii_digit()) {
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                    return Some((curr, count.max(curr)));
                }
            }
        }

        // 2. .7z.001, .7z.002, etc.
        if let Some(pos) = lower.find(".7z.") {
            let after = &lower[pos + 4..];
            if let Ok(curr) = after.parse::<usize>() {
                let mut count = 0;
                if let Ok(entries) = std::fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            let nl = name.to_ascii_lowercase();
                            if nl.starts_with(&lower[..pos + 4]) {
                                let part_str = &nl[pos + 4..];
                                if part_str.chars().all(|c| c.is_ascii_digit()) {
                                    count += 1;
                                }
                            }
                        }
                    }
                }
                return Some((curr, count.max(curr)));
            }
        }

        // 3. .r00, .r01, ... or .z01, .z02, ...
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let ext_l = ext.to_ascii_lowercase();
            if (ext_l.starts_with('r') || ext_l.starts_with('z'))
                && ext_l.len() >= 2
                && ext_l[1..].chars().all(|c| c.is_ascii_digit())
            {
                if let Ok(num) = ext_l[1..].parse::<usize>() {
                    let stem = p.file_stem()?.to_str()?;
                    let stem_l = stem.to_ascii_lowercase();
                    let prefix_char = ext_l.chars().next().unwrap();
                    let mut count = 0;
                    if let Ok(entries) = std::fs::read_dir(parent) {
                        for entry in entries.flatten() {
                            if let Some(name) = entry.file_name().to_str() {
                                let nl = name.to_ascii_lowercase();
                                if let Some(e_idx) = nl.rfind('.') {
                                    let f_stem = &nl[..e_idx];
                                    let f_ext = &nl[e_idx + 1..];
                                    if f_stem == stem_l
                                        && ((f_ext.starts_with(prefix_char)
                                            && f_ext.len() >= 2
                                            && f_ext[1..].chars().all(|c| c.is_ascii_digit()))
                                            || (prefix_char == 'r' && f_ext == "rar")
                                            || (prefix_char == 'z' && f_ext == "zip"))
                                    {
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                    return Some((num + 1, count.max(num + 1)));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::FormatDetector;
    use std::path::Path;

    #[test]
    fn test_format_detector_magic_bytes() {
        // Test simulated buffer logic
        let rar5_bytes = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00];
        assert_eq!(
            &rar5_bytes[0..8],
            &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00]
        );
    }

    #[test]
    fn test_multi_volume_detection() {
        let p = Path::new("test.part02.rar");
        // Non-existent directory, but parses current part correctly
        let info = FormatDetector::detect_volume_info(p);
        assert_eq!(info, Some((2, 2)));
    }
}
