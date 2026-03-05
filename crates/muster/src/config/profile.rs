//! Profile storage: CRUD operations for session templates.
//!
//! A [`Profile`] defines a named tmux session layout — tabs, working directories,
//! startup commands, and pane splits. Profiles are persisted as JSON in the
//! config directory.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Convert a display name into a URL-safe slug for use as a profile ID.
///
/// Lowercase, spaces/underscores become hyphens, non-alphanumeric chars stripped,
/// consecutive hyphens collapsed, leading/trailing hyphens trimmed.
pub fn slugify(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == '_' {
                '-'
            } else {
                c
            }
        })
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect();
    // Collapse consecutive hyphens and trim leading/trailing
    let mut result = String::with_capacity(s.len());
    let mut prev_hyphen = true; // treat start as "previous hyphen" to trim leading
    for c in s.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    // Trim trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }
    result
}

/// A session template defining tabs, colors, and startup behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    /// URL-safe slug identifier (e.g., `"my-project"`).
    pub id: String,
    /// Human-readable display name (e.g., `"My Project"`).
    pub name: String,
    /// Theme color as `#RRGGBB` hex.
    pub color: String,
    /// Ordered list of tabs to create on launch.
    pub tabs: Vec<TabProfile>,
}

/// A tab (tmux window) within a profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TabProfile {
    /// Window name shown in the tmux status bar.
    pub name: String,
    /// Working directory for the window's initial pane.
    pub cwd: String,
    /// Optional startup command sent via `send-keys`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Tmux layout string for multi-pane arrangements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    /// Additional panes beyond the first (created via `split-window`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panes: Vec<PaneProfile>,
}

/// A pane within a tab (created via `split-window`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaneProfile {
    /// Working directory override (falls back to the tab's cwd).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Optional startup command sent via `send-keys`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProfilesFile {
    profiles: HashMap<String, Profile>,
}

/// Manages saved profiles in a config directory.
pub struct ProfileStore {
    config_dir: PathBuf,
}

impl ProfileStore {
    /// Create a new store, ensuring the config directory exists.
    pub fn new(config_dir: &Path) -> Result<Self> {
        fs::create_dir_all(config_dir).map_err(|_| Error::ConfigDir(config_dir.to_path_buf()))?;
        Ok(Self {
            config_dir: config_dir.to_path_buf(),
        })
    }

    fn profiles_path(&self) -> PathBuf {
        self.config_dir.join("profiles.json")
    }

    fn load(&self) -> Result<ProfilesFile> {
        let path = self.profiles_path();
        if !path.exists() {
            return Ok(ProfilesFile::default());
        }
        let data = fs::read_to_string(&path)?;
        let file: ProfilesFile = serde_json::from_str(&data)?;
        Ok(file)
    }

    fn save(&self, file: &ProfilesFile) -> Result<()> {
        let path = self.profiles_path();
        let tmp_path = path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(file)?;
        fs::write(&tmp_path, &data)?;
        fs::rename(&tmp_path, &path)?;
        Ok(())
    }

    /// Create a new profile. Errors if a profile with the same ID exists.
    pub fn create(&self, profile: Profile) -> Result<Profile> {
        let mut file = self.load()?;
        if file.profiles.contains_key(&profile.id) {
            return Err(Error::DuplicateProfile(profile.id));
        }
        file.profiles.insert(profile.id.clone(), profile.clone());
        self.save(&file)?;
        Ok(profile)
    }

    /// List all profiles, sorted by name.
    pub fn list(&self) -> Result<Vec<Profile>> {
        let file = self.load()?;
        let mut profiles: Vec<Profile> = file.profiles.into_values().collect();
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    /// Get a profile by ID. Returns `None` if not found.
    pub fn get(&self, id: &str) -> Result<Option<Profile>> {
        let file = self.load()?;
        Ok(file.profiles.get(id).cloned())
    }

    /// Update an existing profile. Errors if the profile doesn't exist.
    pub fn update(&self, profile: Profile) -> Result<Profile> {
        let mut file = self.load()?;
        if !file.profiles.contains_key(&profile.id) {
            return Err(Error::ProfileNotFound(profile.id));
        }
        file.profiles.insert(profile.id.clone(), profile.clone());
        self.save(&file)?;
        Ok(profile)
    }

    /// Rename a profile: remove the old ID entry and insert with the new ID.
    pub fn rename(&self, old_id: &str, profile: Profile) -> Result<Profile> {
        let mut file = self.load()?;
        if file.profiles.remove(old_id).is_none() {
            return Err(Error::ProfileNotFound(old_id.to_string()));
        }
        file.profiles.insert(profile.id.clone(), profile.clone());
        self.save(&file)?;
        Ok(profile)
    }

    /// Delete a profile by ID. Errors if the profile doesn't exist.
    pub fn delete(&self, id: &str) -> Result<()> {
        let mut file = self.load()?;
        if file.profiles.remove(id).is_none() {
            return Err(Error::ProfileNotFound(id.to_string()));
        }
        self.save(&file)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_profile(id: &str, name: &str) -> Profile {
        Profile {
            id: id.to_string(),
            name: name.to_string(),
            color: "#f97316".to_string(),
            tabs: vec![TabProfile {
                name: "Shell".to_string(),
                cwd: "/tmp".to_string(),
                command: None,
                layout: None,
                panes: vec![],
            }],
        }
    }

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("My Project"), "my-project");
        assert_eq!(slugify("Work"), "work");
        assert_eq!(slugify("hello world"), "hello-world");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("Project @#$ Name!"), "project-name");
        assert_eq!(slugify("foo_bar_baz"), "foo-bar-baz");
        assert_eq!(slugify("a---b"), "a-b");
        assert_eq!(slugify("  leading trailing  "), "leading-trailing");
    }

    #[test]
    fn test_slugify_case() {
        assert_eq!(slugify("UPPER CASE"), "upper-case");
        assert_eq!(slugify("MiXeD CaSe"), "mixed-case");
    }

    #[test]
    fn test_slugify_empty_and_edge() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("---"), "");
        assert_eq!(slugify("a"), "a");
    }

    #[test]
    fn test_create_profile() {
        let dir = TempDir::new().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();
        let profile = test_profile("test-project", "Test Project");

        let created = store.create(profile.clone()).unwrap();
        assert_eq!(created.id, "test-project");
        assert_eq!(created.name, "Test Project");
        assert_eq!(created.tabs.len(), 1);
    }

    #[test]
    fn test_create_duplicate_profile() {
        let dir = TempDir::new().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();
        store
            .create(test_profile("my-project", "My Project"))
            .unwrap();

        let result = store.create(test_profile("my-project", "My Project"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("duplicate profile"));
    }

    #[test]
    fn test_list_profiles() {
        let dir = TempDir::new().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();

        store.create(test_profile("p1", "Beta")).unwrap();
        store.create(test_profile("p2", "Alpha")).unwrap();
        store.create(test_profile("p3", "Gamma")).unwrap();

        let profiles = store.list().unwrap();
        assert_eq!(profiles.len(), 3);
        // Sorted by name
        assert_eq!(profiles[0].name, "Alpha");
        assert_eq!(profiles[1].name, "Beta");
        assert_eq!(profiles[2].name, "Gamma");
    }

    #[test]
    fn test_get_profile() {
        let dir = TempDir::new().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();
        store.create(test_profile("p1", "Test")).unwrap();

        let found = store.get("p1").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Test");

        let missing = store.get("nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_update_profile() {
        let dir = TempDir::new().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();
        store.create(test_profile("p1", "Original")).unwrap();

        let mut updated = test_profile("p1", "Updated");
        updated.color = "#00ff00".to_string();
        updated.tabs.push(TabProfile {
            name: "Server".to_string(),
            cwd: "/home".to_string(),
            command: Some("npm start".to_string()),
            layout: None,
            panes: vec![],
        });

        let result = store.update(updated).unwrap();
        assert_eq!(result.name, "Updated");
        assert_eq!(result.color, "#00ff00");
        assert_eq!(result.tabs.len(), 2);

        // Verify persisted
        let fetched = store.get("p1").unwrap().unwrap();
        assert_eq!(fetched.name, "Updated");
    }

    #[test]
    fn test_update_nonexistent_profile() {
        let dir = TempDir::new().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();

        let result = store.update(test_profile("nonexistent", "Nope"));
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_profile() {
        let dir = TempDir::new().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();
        store.create(test_profile("p1", "To Delete")).unwrap();

        store.delete("p1").unwrap();
        let found = store.get("p1").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_delete_nonexistent_profile() {
        let dir = TempDir::new().unwrap();
        let store = ProfileStore::new(dir.path()).unwrap();

        let result = store.delete("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_profile_persistence() {
        let dir = TempDir::new().unwrap();

        // Create with one store instance
        {
            let store = ProfileStore::new(dir.path()).unwrap();
            store.create(test_profile("p1", "Persistent")).unwrap();
        }

        // Read with a new store instance
        {
            let store = ProfileStore::new(dir.path()).unwrap();
            let found = store.get("p1").unwrap();
            assert!(found.is_some());
            assert_eq!(found.unwrap().name, "Persistent");
        }
    }

    #[test]
    fn test_profile_serde_roundtrip() {
        let profile = Profile {
            id: "profile_abc".to_string(),
            name: "Roundtrip Test".to_string(),
            color: "#abcdef".to_string(),
            tabs: vec![
                TabProfile {
                    name: "Shell".to_string(),
                    cwd: "/tmp".to_string(),
                    command: None,
                    layout: None,
                    panes: vec![],
                },
                TabProfile {
                    name: "Server".to_string(),
                    cwd: "/home/user/app".to_string(),
                    command: Some("npm run dev".to_string()),
                    layout: None,
                    panes: vec![],
                },
            ],
        };

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, deserialized);
    }

    #[test]
    fn test_pane_profile_serde_roundtrip() {
        let profile = Profile {
            id: "pane_test".to_string(),
            name: "Pane Layout Test".to_string(),
            color: "#00ff00".to_string(),
            tabs: vec![TabProfile {
                name: "Dev".to_string(),
                cwd: "/home/user/project".to_string(),
                command: None,
                layout: Some("5a4a,204x51,0,0{102x51,0,0,0,101x51,103,0,1}".to_string()),
                panes: vec![
                    PaneProfile {
                        cwd: Some("/home/user/project/src".to_string()),
                        command: Some("vim .".to_string()),
                    },
                    PaneProfile {
                        cwd: Some("/home/user/project".to_string()),
                        command: Some("cargo watch".to_string()),
                    },
                ],
            }],
        };

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, deserialized);

        // Verify pane fields survived the roundtrip
        let tab = &deserialized.tabs[0];
        assert_eq!(tab.panes.len(), 2);
        assert_eq!(
            tab.layout.as_deref(),
            Some("5a4a,204x51,0,0{102x51,0,0,0,101x51,103,0,1}")
        );
        assert_eq!(
            tab.panes[0].cwd.as_deref(),
            Some("/home/user/project/src")
        );
        assert_eq!(tab.panes[0].command.as_deref(), Some("vim ."));
        assert_eq!(
            tab.panes[1].cwd.as_deref(),
            Some("/home/user/project")
        );
        assert_eq!(tab.panes[1].command.as_deref(), Some("cargo watch"));
    }

    #[test]
    fn test_tab_profile_backward_compat_no_layout_no_panes() {
        // JSON from before panes/layout were added — missing both fields entirely
        let json = r##"{
            "id": "old_profile",
            "name": "Legacy Profile",
            "color": "#abcdef",
            "tabs": [
                {
                    "name": "Shell",
                    "cwd": "/tmp",
                    "command": "echo hi"
                }
            ]
        }"##;

        let profile: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.tabs.len(), 1);

        let tab = &profile.tabs[0];
        assert_eq!(tab.name, "Shell");
        assert_eq!(tab.cwd, "/tmp");
        assert_eq!(tab.command.as_deref(), Some("echo hi"));
        assert!(tab.layout.is_none(), "layout should default to None");
        assert!(tab.panes.is_empty(), "panes should default to empty vec");
    }

    #[test]
    fn test_pane_profile_optional_fields() {
        // PaneProfile with no cwd, no command
        let pane = PaneProfile {
            cwd: None,
            command: None,
        };
        let json = serde_json::to_string(&pane).unwrap();
        // skip_serializing_if means empty fields are omitted
        assert_eq!(json, "{}");

        let deserialized: PaneProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, pane);
    }

    #[test]
    fn test_tab_profile_empty_panes_not_serialized() {
        let tab = TabProfile {
            name: "Shell".to_string(),
            cwd: "/tmp".to_string(),
            command: None,
            layout: None,
            panes: vec![],
        };
        let json = serde_json::to_string(&tab).unwrap();
        // panes should be omitted when empty (skip_serializing_if = "Vec::is_empty")
        assert!(
            !json.contains("panes"),
            "empty panes should not appear in JSON: {json}"
        );
        // layout should be omitted when None
        assert!(
            !json.contains("layout"),
            "None layout should not appear in JSON: {json}"
        );
    }
}
