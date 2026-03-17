//! Global settings: tmux path, shell, and terminal emulator preferences.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Global configuration for the muster runtime.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    /// Explicit path to the tmux binary. Discovered via PATH if not set.
    #[serde(default)]
    pub tmux_path: Option<String>,
    /// Shell to use for new tmux panes. Defaults to `$SHELL`.
    #[serde(default)]
    pub shell: Option<String>,
    /// Terminal emulator for new windows and notification click-to-source.
    /// Examples: "ghostty", "alacritty", "kitty", "wezterm", "terminal", "iterm2".
    /// Default: platform default (Terminal.app on macOS, auto-detected on Linux).
    #[serde(default)]
    pub terminal: Option<String>,
}

/// Manages settings.json in the config directory.
pub struct SettingsStore {
    config_dir: PathBuf,
}

impl SettingsStore {
    /// Create a new store, ensuring the config directory exists.
    pub fn new(config_dir: &Path) -> Result<Self> {
        fs::create_dir_all(config_dir).map_err(|_| Error::ConfigDir(config_dir.to_path_buf()))?;
        Ok(Self {
            config_dir: config_dir.to_path_buf(),
        })
    }

    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    /// Load settings from disk, returning defaults if the file doesn't exist.
    pub fn load(&self) -> Result<Settings> {
        let path = self.settings_path();
        if !path.exists() {
            return Ok(Settings::default());
        }
        let data = fs::read_to_string(&path)?;
        let settings: Settings = serde_json::from_str(&data)?;
        Ok(settings)
    }

    /// Persist settings to disk (atomic write via temp file + rename).
    pub fn save(&self, settings: &Settings) -> Result<()> {
        let path = self.settings_path();
        let tmp_path = path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(settings)?;
        fs::write(&tmp_path, &data)?;
        fs::rename(&tmp_path, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_settings() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::new(dir.path()).unwrap();

        let settings = store.load().unwrap();
        assert!(settings.tmux_path.is_none());
        assert!(settings.shell.is_none());
    }

    #[test]
    fn test_save_and_load_settings() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::new(dir.path()).unwrap();

        let settings = Settings {
            tmux_path: Some("/opt/homebrew/bin/tmux".to_string()),
            shell: Some("/usr/local/bin/fish".to_string()),
            terminal: Some("ghostty".to_string()),
        };

        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(settings, loaded);
    }

    #[test]
    fn test_settings_serde_roundtrip() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, deserialized);
    }

    #[test]
    fn test_settings_partial_json() {
        // Settings should use defaults for missing fields
        let json = r#"{"tmux_path": "/usr/bin/tmux"}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.tmux_path.as_deref(), Some("/usr/bin/tmux"));
        assert!(settings.shell.is_none());
    }

    #[test]
    fn test_snapshot_settings_default() {
        insta::assert_json_snapshot!(Settings::default());
    }

    #[test]
    fn test_snapshot_settings_populated() {
        let settings = Settings {
            tmux_path: Some("/opt/homebrew/bin/tmux".to_string()),
            shell: Some("/bin/zsh".to_string()),
            terminal: Some("ghostty".to_string()),
        };
        insta::assert_json_snapshot!(settings);
    }
}
