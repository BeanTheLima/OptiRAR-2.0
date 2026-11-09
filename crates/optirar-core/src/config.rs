use crate::error::{OptiError, Result};
use crate::i18n::Language;
use crate::types::CompressionProfile;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn display_name(&self) -> &'static str {
        match self {
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
            ThemeMode::System => "System",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)] // configs from older versions keep loading when fields are added
pub struct AppConfig {
    pub language: Language,
    pub theme: ThemeMode,
    pub default_compression_profile: CompressionProfile,
    pub default_destination: Option<PathBuf>,
    pub smart_extract: bool,
    pub open_folder_after_extract: bool,
    pub thread_count: usize,
    pub recent_archives: Vec<PathBuf>,
    pub shell_integration_enabled: bool,
    pub remember_window_geometry: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub confirm_delete: bool,
    pub auto_check_integrity: bool,
    /// Code page for ZIP names without the UTF-8 flag (0 = automatic).
    pub zip_name_codepage: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::EnUs,
            theme: ThemeMode::Dark,
            default_compression_profile: CompressionProfile::Balanced,
            default_destination: None,
            smart_extract: true,
            open_folder_after_extract: true,
            thread_count: 0, // 0 = automatic (physical cores)
            recent_archives: Vec::new(),
            shell_integration_enabled: false,
            remember_window_geometry: true,
            window_width: 1100.0,
            window_height: 720.0,
            confirm_delete: true,
            auto_check_integrity: true,
            zip_name_codepage: 0,
        }
    }
}

impl AppConfig {
    /// Return the configuration directory: `%APPDATA%\OptiRAR` on Windows,
    /// or `~/.config/optirar` on other platforms.
    pub fn config_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return PathBuf::from(appdata).join("OptiRAR");
            }
        }

        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            return PathBuf::from(home).join(".config").join("optirar");
        }

        PathBuf::from(".optirar")
    }

    /// Path to `settings.json`.
    pub fn config_file_path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    /// Load configuration from disk, or return default if missing or invalid.
    pub fn load() -> Self {
        let path = Self::config_file_path();
        if path.exists() {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&contents) {
                    return config;
                }
            }
        }
        Self::default()
    }

    /// Persist configuration to disk as formatted JSON.
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        let path = Self::config_file_path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| OptiError::Internal(format!("Failed to serialize config: {}", e)))?;

        // Write directly with safe fallback
        fs::write(&path, json)?;

        Ok(())
    }

    /// Add an archive path to the front of recent archives list, deduplicating and limiting to 20.
    pub fn add_recent<P: AsRef<Path>>(&mut self, path: P) {
        let p = path.as_ref().to_path_buf();
        self.recent_archives.retain(|existing| existing != &p);
        self.recent_archives.insert(0, p);
        if self.recent_archives.len() > 20 {
            self.recent_archives.truncate(20);
        }
    }

    /// Remove a path from recent archives.
    pub fn remove_recent<P: AsRef<Path>>(&mut self, path: P) {
        let p = path.as_ref();
        self.recent_archives.retain(|existing| existing != p);
    }

    /// Clear all recent archives.
    pub fn clear_recent(&mut self) {
        self.recent_archives.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.language, Language::EnUs);
        assert_eq!(cfg.theme, ThemeMode::Dark);
        assert!(cfg.smart_extract);
        assert_eq!(cfg.recent_archives.len(), 0);
    }

    #[test]
    fn test_add_recent() {
        let mut cfg = AppConfig::default();
        cfg.add_recent("test1.zip");
        cfg.add_recent("test2.zip");
        cfg.add_recent("test1.zip");

        assert_eq!(cfg.recent_archives.len(), 2);
        assert_eq!(cfg.recent_archives[0], PathBuf::from("test1.zip"));
        assert_eq!(cfg.recent_archives[1], PathBuf::from("test2.zip"));
    }
}
