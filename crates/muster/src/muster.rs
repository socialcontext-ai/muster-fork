use std::path::{Path, PathBuf};

use tokio::sync::broadcast;

use crate::config::profile::{Profile, ProfileStore};
use crate::config::settings::{Settings, SettingsStore};
use crate::emulator::{Emulator, GhosttyEmulator};
use crate::error::{Error, Result};
use crate::session;
use crate::tmux::client::TmuxClient;
use crate::tmux::control::MusterEvent;
use crate::tmux::types::SessionInfo;

/// Main facade for the muster library.
///
/// Ties together tmux interaction, profile management, settings, and emulator control.
pub struct Muster {
    client: TmuxClient,
    profiles: ProfileStore,
    settings: SettingsStore,
    emulator: Box<dyn Emulator>,
    config_dir: PathBuf,
    tx: broadcast::Sender<MusterEvent>,
}

impl Muster {
    /// Initialize the muster library.
    ///
    /// Discovers tmux, loads config, and prepares for operation.
    pub fn init(config_dir: &Path) -> Result<Self> {
        let settings_store = SettingsStore::new(config_dir)?;
        let settings = settings_store.load()?;

        let client = if let Some(ref path) = settings.tmux_path {
            TmuxClient::with_path(PathBuf::from(path))
        } else {
            TmuxClient::new()?
        };

        let profiles = ProfileStore::new(config_dir)?;

        // TODO: match on settings.emulator when more emulators are supported
        let emulator: Box<dyn Emulator> = Box::new(GhosttyEmulator::new());

        let (tx, _) = broadcast::channel(64);

        Ok(Self {
            client,
            profiles,
            settings: settings_store,
            emulator,
            config_dir: config_dir.to_path_buf(),
            tx,
        })
    }

    /// Initialize with custom settings (for testing).
    pub fn init_with_settings(config_dir: &Path, settings: &Settings) -> Result<Self> {
        let settings_store = SettingsStore::new(config_dir)?;
        settings_store.save(settings)?;

        let client = if let Some(ref path) = settings.tmux_path {
            TmuxClient::with_path(PathBuf::from(path))
        } else {
            TmuxClient::new()?
        };

        let profiles = ProfileStore::new(config_dir)?;
        let emulator: Box<dyn Emulator> = Box::new(GhosttyEmulator::new());
        let (tx, _) = broadcast::channel(64);

        Ok(Self {
            client,
            profiles,
            settings: settings_store,
            emulator,
            config_dir: config_dir.to_path_buf(),
            tx,
        })
    }

    // --- Profiles ---

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.profiles.list()
    }

    pub fn get_profile(&self, id: &str) -> Result<Option<Profile>> {
        self.profiles.get(id)
    }

    pub fn save_profile(&self, profile: Profile) -> Result<Profile> {
        self.profiles.create(profile)
    }

    pub fn update_profile(&self, profile: Profile) -> Result<Profile> {
        self.profiles.update(profile)
    }

    pub fn delete_profile(&self, id: &str) -> Result<()> {
        self.profiles.delete(id)
    }

    // --- Sessions ---

    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        self.client.list_sessions_with_metadata()
    }

    pub fn launch(&self, profile_id: &str) -> Result<SessionInfo> {
        let profile = self
            .profiles
            .get(profile_id)?
            .ok_or_else(|| Error::ProfileNotFound(profile_id.to_string()))?;

        let session_name = format!("muster_{}", profile.id);

        // If session already exists, return its info
        if self.client.has_session(&session_name)? {
            let sessions = self.client.list_sessions_with_metadata()?;
            if let Some(info) = sessions
                .into_iter()
                .find(|s| s.session_name == session_name)
            {
                return Ok(info);
            }
        }

        // Resolve shell from settings → $SHELL
        let settings = self.settings.load()?;
        let shell = session::resolve_shell(settings.shell.as_deref());

        // Create from profile
        let info = session::create_from_profile(&self.client, &profile, shell.as_deref())?;

        // Apply theme
        session::theme::apply_theme(
            &self.client,
            &info.session_name,
            &profile.color,
            &profile.name,
        )?;

        Ok(info)
    }

    /// Resolve a user-provided identifier to a tmux session name.
    ///
    /// Tries `muster_{input}` first (the common case for profile-based sessions),
    /// then the literal input as a fallback.
    pub fn resolve_session(&self, input: &str) -> Result<String> {
        let prefixed = format!("muster_{input}");
        if self.client.has_session(&prefixed)? {
            return Ok(prefixed);
        }
        if self.client.has_session(input)? {
            return Ok(input.to_string());
        }
        Err(Error::SessionNotFound(input.to_string()))
    }

    pub fn destroy(&self, session_name: &str) -> Result<()> {
        session::destroy(&self.client, session_name)
    }

    // --- Windows ---

    pub fn add_window(
        &self,
        session: &str,
        name: &str,
        cwd: &str,
        command: Option<&str>,
    ) -> Result<()> {
        let settings = self.settings.load()?;
        let shell = session::resolve_shell(settings.shell.as_deref());
        self.client
            .new_window(session, name, cwd, shell.as_deref())?;
        if let Some(cmd) = command {
            let windows = self.client.list_windows(session)?;
            if let Some(last) = windows.last() {
                self.client.send_keys(session, last.index, cmd)?;
            }
        }
        Ok(())
    }

    pub fn close_window(&self, session: &str, window_index: u32) -> Result<()> {
        self.client.kill_window(session, window_index)
    }

    pub fn switch_window(&self, session: &str, window_index: u32) -> Result<()> {
        self.client.select_window(session, window_index)
    }

    pub fn rename_window(&self, session: &str, window_index: u32, name: &str) -> Result<()> {
        self.client.rename_window(session, window_index, name)
    }

    // --- Theme ---

    pub fn set_color(&self, session: &str, color: &str) -> Result<()> {
        // Get display name for theme application
        let display_name = self
            .client
            .get_option(session, "@muster_name")?
            .unwrap_or_else(|| session.to_string());

        // set_color resolves named colors internally
        session::theme::set_color(&self.client, session, color, &display_name)?;

        // Persist to profile if this is a muster-managed session
        if let Some(profile_id) = session.strip_prefix("muster_") {
            if let Some(mut profile) = self.profiles.get(profile_id)? {
                profile.color = session::theme::resolve_color(color)?;
                self.profiles.update(profile)?;
            }
        }

        Ok(())
    }

    // --- Emulator ---

    pub fn open_emulator(&self, session: &str) -> Result<()> {
        self.emulator.launch(session)?;
        Ok(())
    }

    // --- Events ---

    pub fn subscribe(&self) -> broadcast::Receiver<MusterEvent> {
        self.tx.subscribe()
    }

    // --- Accessors ---

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn client(&self) -> &TmuxClient {
        &self.client
    }

    pub fn settings(&self) -> Result<Settings> {
        self.settings.load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_creates_config_dir() {
        let dir = TempDir::new().unwrap();
        let config_dir = dir.path().join("muster_config");

        // Config dir doesn't exist yet
        assert!(!config_dir.exists());

        // Init should create it (will fail on tmux discovery if tmux not installed,
        // but the dir should still be created by the stores)
        let result = Muster::init(&config_dir);
        // Even if tmux isn't found, the config dir should have been created
        // by ProfileStore and SettingsStore
        assert!(config_dir.exists());

        // If tmux is installed, verify the full init works
        if result.is_ok() {
            let m = result.unwrap();
            assert_eq!(m.config_dir(), config_dir);
        }
    }

    #[test]
    #[ignore]
    fn test_full_lifecycle() {
        let dir = TempDir::new().unwrap();
        let m = Muster::init(dir.path()).expect("init");

        // Create a profile
        let profile = Profile {
            id: format!("test_{}", uuid::Uuid::new_v4()),
            name: "Lifecycle Test".to_string(),
            color: "#ff6600".to_string(),
            tabs: vec![crate::config::profile::TabProfile {
                name: "Shell".to_string(),
                cwd: "/tmp".to_string(),
                command: None,
            }],
        };

        let saved = m.save_profile(profile.clone()).unwrap();
        assert_eq!(saved.name, "Lifecycle Test");

        // List profiles
        let profiles = m.list_profiles().unwrap();
        assert!(profiles.iter().any(|p| p.id == saved.id));

        // Launch session from profile
        let info = m.launch(&saved.id).unwrap();
        assert_eq!(info.display_name, "Lifecycle Test");

        // List sessions
        let sessions = m.list_sessions().unwrap();
        assert!(sessions.iter().any(|s| s.session_name == info.session_name));

        // Destroy session
        m.destroy(&info.session_name).unwrap();
        let sessions = m.list_sessions().unwrap();
        assert!(!sessions.iter().any(|s| s.session_name == info.session_name));

        // Delete profile
        m.delete_profile(&saved.id).unwrap();
    }
}
