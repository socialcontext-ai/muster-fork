use std::path::{Path, PathBuf};

use tokio::sync::broadcast;

use crate::config::profile::{PaneProfile, Profile, ProfileStore, TabProfile};
use crate::config::settings::{Settings, SettingsStore};
use crate::error::{Error, Result};
use crate::session;
use crate::tmux::client::TmuxClient;
use crate::tmux::control::MusterEvent;
use crate::tmux::types::{PaneContext, SessionInfo};

/// Result of a `pin` operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinResult {
    /// Window was newly pinned to the profile.
    Pinned,
    /// Window was already pinned; layout was updated.
    LayoutUpdated,
    /// Window was already pinned; layout unchanged.
    AlreadyCurrent,
}

/// Main facade for the muster library.
///
/// Ties together tmux interaction, profile management, settings, and emulator control.
pub struct Muster {
    client: TmuxClient,
    profiles: ProfileStore,
    settings: SettingsStore,
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

        let (tx, _) = broadcast::channel(64);

        Ok(Self {
            client,
            profiles,
            settings: settings_store,
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
        let (tx, _) = broadcast::channel(64);

        Ok(Self {
            client,
            profiles,
            settings: settings_store,
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

    pub fn rename_profile(&self, old_id: &str, profile: Profile) -> Result<Profile> {
        self.profiles.rename(old_id, profile)
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

    // --- Pin / Unpin ---

    /// Count pinned windows in `session` whose index is less than `window_index`.
    /// This gives the profile tab position for the window.
    fn pinned_rank(&self, session: &str, window_index: u32) -> Result<usize> {
        let windows = self.client.list_windows(session)?;
        let count = windows
            .iter()
            .filter(|w| w.index < window_index)
            .filter(|w| {
                self.client
                    .get_window_option(session, w.index, "@muster_pinned")
                    .ok()
                    .flatten()
                    .is_some()
            })
            .count();
        Ok(count)
    }

    /// Resolve the pane context for the current tmux pane (reads `$TMUX_PANE`).
    pub fn resolve_current_pane(&self) -> Result<PaneContext> {
        let pane_id = std::env::var("TMUX_PANE").map_err(|_| Error::NotInTmux)?;
        self.client.resolve_pane_context(&pane_id)
    }

    /// Capture the current pane layout for a window and return (layout, panes).
    ///
    /// If the window has only one pane, returns `(None, vec![])`.
    fn capture_window_layout(
        &self,
        session: &str,
        window_index: u32,
    ) -> Result<(Option<String>, Vec<PaneProfile>)> {
        let tmux_panes = self.client.list_window_panes(session, window_index)?;
        if tmux_panes.len() <= 1 {
            return Ok((None, vec![]));
        }
        let layout = self.client.get_window_layout(session, window_index)?;
        let layout = if layout.is_empty() {
            None
        } else {
            Some(layout)
        };
        let panes = tmux_panes
            .iter()
            .map(|p| PaneProfile {
                cwd: Some(p.cwd.clone()),
                command: None,
            })
            .collect();
        Ok((layout, panes))
    }

    /// Pin the current window to the session's profile.
    ///
    /// First pin adds the window. Re-pin updates the pane layout.
    pub fn pin_window(&self) -> Result<PinResult> {
        let ctx = self.resolve_current_pane()?;

        // Must be a muster-managed session
        let profile_id = self
            .client
            .get_option(&ctx.session_name, "@muster_profile")?
            .ok_or(Error::NotMusterSession)?;

        // Already pinned — re-pin to update layout
        if self
            .client
            .get_window_option(&ctx.session_name, ctx.window_index, "@muster_pinned")?
            .is_some()
        {
            return self.update_pinned_layout(&ctx, &profile_id);
        }

        // Capture pane layout for the new pin
        let (layout, panes) =
            self.capture_window_layout(&ctx.session_name, ctx.window_index)?;

        // Insert tab at the position matching its rank among pinned windows
        let mut profile = self
            .profiles
            .get(&profile_id)?
            .ok_or_else(|| Error::ProfileNotFound(profile_id.clone()))?;

        let insert_pos = self
            .pinned_rank(&ctx.session_name, ctx.window_index)?
            .min(profile.tabs.len());
        profile.tabs.insert(
            insert_pos,
            TabProfile {
                name: ctx.window_name.clone(),
                cwd: ctx.cwd.clone(),
                command: None,
                layout,
                panes,
            },
        );
        self.profiles.update(profile)?;

        // Apply colored styling
        let color = self
            .client
            .get_option(&ctx.session_name, "@muster_color")?
            .unwrap_or_else(|| "#808080".to_string());
        let display_name = self
            .client
            .get_option(&ctx.session_name, "@muster_name")?
            .unwrap_or_else(|| ctx.session_name.clone());

        session::theme::apply_pinned_window_style(
            &self.client,
            &ctx.session_name,
            ctx.window_index,
            &color,
            &display_name,
        )?;

        // Track the tab name so rename sync can find the right profile entry
        self.client.set_window_option(
            &ctx.session_name,
            ctx.window_index,
            "@muster_tab_name",
            &ctx.window_name,
        )?;

        Ok(PinResult::Pinned)
    }

    /// Update the pane layout for an already-pinned window.
    ///
    /// Captures the current pane geometry from tmux and updates the profile.
    /// Preserves existing per-pane commands where pane indices match.
    fn update_pinned_layout(
        &self,
        ctx: &PaneContext,
        profile_id: &str,
    ) -> Result<PinResult> {
        let (new_layout, new_panes) =
            self.capture_window_layout(&ctx.session_name, ctx.window_index)?;

        let mut profile = self
            .profiles
            .get(profile_id)?
            .ok_or_else(|| Error::ProfileNotFound(profile_id.to_string()))?;

        let tab_pos = self.pinned_rank(&ctx.session_name, ctx.window_index)?;
        let Some(tab) = profile.tabs.get_mut(tab_pos) else {
            return Ok(PinResult::AlreadyCurrent);
        };

        // Check if anything actually changed
        if tab.layout == new_layout && tab.panes.len() == new_panes.len() {
            let cwds_match = tab
                .panes
                .iter()
                .zip(new_panes.iter())
                .all(|(old, new)| old.cwd == new.cwd);
            if cwds_match {
                // Clear the stale indicator even if nothing changed
                let _ = self.client.unset_window_option(
                    &ctx.session_name,
                    ctx.window_index,
                    "@muster_layout_stale",
                );
                return Ok(PinResult::AlreadyCurrent);
            }
        }

        // Merge: preserve existing per-pane commands where indices match
        let merged_panes: Vec<PaneProfile> = new_panes
            .into_iter()
            .enumerate()
            .map(|(i, mut new_pane)| {
                if let Some(old_pane) = tab.panes.get(i) {
                    if new_pane.command.is_none() {
                        new_pane.command = old_pane.command.clone();
                    }
                }
                new_pane
            })
            .collect();

        tab.layout = new_layout;
        tab.panes = merged_panes;

        self.profiles.update(profile)?;

        // Clear the stale indicator now that the layout is saved
        let _ = self.client.unset_window_option(
            &ctx.session_name,
            ctx.window_index,
            "@muster_layout_stale",
        );

        Ok(PinResult::LayoutUpdated)
    }

    /// Unpin the current window from the session's profile.
    pub fn unpin_window(&self) -> Result<()> {
        let ctx = self.resolve_current_pane()?;

        // Must be a muster-managed session
        let profile_id = self
            .client
            .get_option(&ctx.session_name, "@muster_profile")?
            .ok_or(Error::NotMusterSession)?;

        // Not pinned — no-op
        if self
            .client
            .get_window_option(&ctx.session_name, ctx.window_index, "@muster_pinned")?
            .is_none()
        {
            return Ok(());
        }

        // Remove tab by positional rank (avoids ambiguity when names collide)
        let mut profile = self
            .profiles
            .get(&profile_id)?
            .ok_or_else(|| Error::ProfileNotFound(profile_id.clone()))?;

        let tab_pos = self.pinned_rank(&ctx.session_name, ctx.window_index)?;
        if tab_pos < profile.tabs.len() {
            profile.tabs.remove(tab_pos);
        }
        self.profiles.update(profile)?;

        // Apply neutral styling (also removes @muster_pinned)
        session::theme::apply_neutral_window_style(
            &self.client,
            &ctx.session_name,
            ctx.window_index,
        )?;

        Ok(())
    }

    // --- Rename sync ---

    /// Sync a window rename to the profile. Called by the `after-rename-window` hook.
    pub fn sync_rename(&self, session: &str, window_index: u32, new_name: &str) -> Result<()> {
        // Must be a muster-managed session with a profile
        let Some(profile_id) = self.client.get_option(session, "@muster_profile")? else {
            return Ok(());
        };

        // Only sync pinned windows
        if self
            .client
            .get_window_option(session, window_index, "@muster_pinned")?
            .is_none()
        {
            return Ok(());
        }

        // Find the tab by positional rank (avoids ambiguity when names collide)
        let tab_pos = self.pinned_rank(session, window_index)?;

        let Some(mut profile) = self.profiles.get(&profile_id)? else {
            return Ok(());
        };

        if let Some(tab) = profile.tabs.get_mut(tab_pos) {
            if tab.name == new_name {
                return Ok(());
            }
            tab.name = new_name.to_string();
        }
        self.profiles.update(profile)?;

        // Keep the stored tab name in sync
        self.client
            .set_window_option(session, window_index, "@muster_tab_name", new_name)?;

        Ok(())
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

    fn ensure_anchor() {
        let Ok(client) = crate::TmuxClient::new() else { return };
        let _ = client.new_session("muster_test_anchor", "anchor", "/tmp", None);
        let _ = client.cmd(&["set-option", "-s", "exit-empty", "off"]);
    }

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
        ensure_anchor();
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
                layout: None,
                panes: vec![],
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
