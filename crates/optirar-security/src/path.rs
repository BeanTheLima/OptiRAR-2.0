use optirar_core::error::{OptiError, Result};
use std::path::{Component, Path, PathBuf};

/// Sanitizes archive entry paths and guards against directory traversal (Zip Slip) attacks.
pub struct PathSanitizer;

impl PathSanitizer {
    /// Sanitizes an entry path from an archive into a safe relative path.
    ///
    /// Defenses implemented:
    /// - Strips Windows drive volume identifiers (`C:`, `D:`)
    /// - Rejects parent directory traversal (`..`)
    /// - Strips leading slashes and backslashes
    /// - Replaces illegal Windows filename characters
    /// - Rejects Windows reserved device names (CON, PRN, AUX, NUL, COM1-9, LPT1-9)
    pub fn sanitize(raw_path: &str) -> Result<PathBuf> {
        let normalized = raw_path.replace('\\', "/");
        let trimmed = normalized.trim_start_matches('/');

        let mut safe_path = PathBuf::new();

        for component in Path::new(trimmed).components() {
            match component {
                Component::Prefix(_) => {
                    // Windows drive prefix like C: or \\server\share
                    return Err(OptiError::PathTraversalDetected(format!(
                        "Drive prefix detected in entry: '{}'",
                        raw_path
                    )));
                }
                Component::RootDir => {
                    // Leading root slash
                    continue;
                }
                Component::CurDir => {
                    // Current dir '.'
                    continue;
                }
                Component::ParentDir => {
                    // Traversal '..'
                    return Err(OptiError::PathTraversalDetected(format!(
                        "Parent directory traversal '..' detected in entry: '{}'",
                        raw_path
                    )));
                }
                Component::Normal(os_str) => {
                    let seg = os_str.to_string_lossy();
                    // Reject Windows reserved device names
                    if is_reserved_dos_name(&seg) {
                        return Err(OptiError::PathTraversalDetected(format!(
                            "Reserved Windows device name detected: '{}'",
                            seg
                        )));
                    }
                    // Sanitize illegal Windows filename characters
                    let sanitized_seg = sanitize_filename_segment(&seg);
                    safe_path.push(sanitized_seg);
                }
            }
        }

        if safe_path.as_os_str().is_empty() {
            return Err(OptiError::PathTraversalDetected(
                "Entry path resolved to empty target".to_string(),
            ));
        }

        Ok(safe_path)
    }

    /// Resolve a sanitized target path inside `target_dir`, ensuring the canonical path
    /// remains strictly within the intended destination directory.
    pub fn resolve_destination(target_dir: &Path, raw_entry_path: &str) -> Result<PathBuf> {
        let safe_relative = Self::sanitize(raw_entry_path)?;
        let full_path = target_dir.join(&safe_relative);

        // Security check: ensure the assembled path is a child of target_dir
        if !full_path.starts_with(target_dir) {
            return Err(OptiError::PathTraversalDetected(format!(
                "Escaped destination root: '{}'",
                raw_entry_path
            )));
        }

        Ok(full_path)
    }
}

/// Checks if a filename component matches reserved DOS device names.
fn is_reserved_dos_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// Replaces characters that are illegal on NTFS/Windows filesystems.
fn sanitize_filename_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            '\0'..='\x1f' => '_',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_sanitizer_normal() {
        let p = PathSanitizer::sanitize("folder/subfolder/file.txt").unwrap();
        assert_eq!(p, PathBuf::from("folder/subfolder/file.txt"));
    }

    #[test]
    fn test_path_sanitizer_windows_slashes() {
        let p = PathSanitizer::sanitize(r"folder\subfolder\file.txt").unwrap();
        assert_eq!(p, PathBuf::from("folder/subfolder/file.txt"));
    }

    #[test]
    fn test_path_sanitizer_rejects_parent_dir() {
        let res = PathSanitizer::sanitize("../escape.txt");
        assert!(res.is_err());

        let res2 = PathSanitizer::sanitize("folder/../../escape.txt");
        assert!(res2.is_err());
    }

    #[test]
    fn test_path_sanitizer_rejects_reserved_dos_names() {
        let res = PathSanitizer::sanitize("NUL");
        assert!(res.is_err());

        let res2 = PathSanitizer::sanitize("folder/aux.txt");
        assert!(res2.is_err());
    }
}
