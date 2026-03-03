use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(default = "default_emulator")]
    pub emulator: String,
    #[serde(default)]
    pub emulator_path: Option<String>,
    #[serde(default)]
    pub tmux_path: Option<String>,
    /// Shell to use for new tmux panes. Defaults to `$SHELL`.
    #[serde(default)]
    pub shell: Option<String>,
}

fn default_emulator() -> String {
    "ghostty".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            emulator: default_emulator(),
            emulator_path: None,
            tmux_path: None,
            shell: None,
        }
    }
}

/// Manages settings.json in the config directory.
pub struct SettingsStore {
    config_dir: PathBuf,
}

impl SettingsStore {
    pub fn new(config_dir: &Path) -> Result<Self> {
        fs::create_dir_all(config_dir).map_err(|_| Error::ConfigDir(config_dir.to_path_buf()))?;
        Ok(Self {
            config_dir: config_dir.to_path_buf(),
        })
    }

    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn load(&self) -> Result<Settings> {
        let path = self.settings_path();
        if !path.exists() {
            return Ok(Settings::default());
        }
        let data = fs::read_to_string(&path)?;
        let settings: Settings = serde_json::from_str(&data)?;
        Ok(settings)
    }

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
        assert_eq!(settings.emulator, "ghostty");
        assert!(settings.emulator_path.is_none());
        assert!(settings.tmux_path.is_none());
    }

    #[test]
    fn test_save_and_load_settings() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::new(dir.path()).unwrap();

        let settings = Settings {
            emulator: "alacritty".to_string(),
            emulator_path: Some("/usr/local/bin/alacritty".to_string()),
            tmux_path: Some("/opt/homebrew/bin/tmux".to_string()),
            shell: None,
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
        let json = r#"{"emulator": "kitty"}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.emulator, "kitty");
        assert!(settings.emulator_path.is_none());
        assert!(settings.tmux_path.is_none());
    }
}
