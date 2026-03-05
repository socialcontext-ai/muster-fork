//! Configuration management: profiles and settings.
//!
//! Profiles define tmux session templates (tabs, panes, commands).
//! Settings control global behavior (tmux path, shell, terminal emulator).

pub mod profile;
pub mod settings;

pub use profile::{PaneProfile, Profile, ProfileStore, TabProfile};
pub use settings::{Settings, SettingsStore};
