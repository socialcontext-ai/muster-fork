//! CLI type definitions for muster.
//!
//! This module defines the clap command structure used by the muster binary.
//! It is exposed as a library target so that tools like `clap-markdown` can
//! generate documentation from the type definitions.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Terminal session group management built on tmux.
///
/// Muster organizes terminal sessions into named, color-coded groups with saved
/// profiles, runtime theming, and push-based state synchronization via tmux
/// control mode.
#[derive(Parser)]
#[command(name = "muster", version, about = "Terminal session group management")]
pub struct Cli {
    /// Path to the config directory
    #[arg(long, env = "MUSTER_CONFIG_DIR")]
    pub config_dir: Option<PathBuf>,

    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Subcommand)]
pub enum Command {
    /// List profiles and running sessions
    List,

    /// Create or attach to a profile's session
    #[command(alias = "launch")]
    Up {
        /// Profile name or ID
        profile: String,
        /// Switch to this tab index on attach
        #[arg(long)]
        tab: Option<u32>,
        /// Create session but don't attach
        #[arg(long)]
        detach: bool,
    },

    /// Attach to a running session
    #[command(hide = true)]
    Attach {
        /// Profile name, ID, or session name
        session: String,
        /// Tab index to switch to
        #[arg(long)]
        tab: Option<u32>,
    },

    /// Destroy a session
    #[command(alias = "kill")]
    Down {
        /// Profile name, ID, or session name
        session: String,
    },

    /// Create an ad-hoc session
    New {
        /// Display name
        name: String,
        /// Tab definition (`name:cwd[:command]`), repeatable
        #[arg(long)]
        tab: Vec<String>,
        /// Color (hex)
        #[arg(long, default_value = "#808080")]
        color: String,
        /// Create session but don't attach
        #[arg(long)]
        detach: bool,
    },

    /// Manage session colors
    Color {
        /// Profile name, ID, or session name
        session: Option<String>,
        /// New color (hex or named)
        color: Option<String>,
        /// List available named colors
        #[arg(long)]
        list: bool,
    },

    /// Show processes running inside sessions
    Ps {
        /// Profile name or ID (shows all sessions if omitted)
        profile: Option<String>,
    },

    /// Show listening ports inside sessions
    Ports {
        /// Profile name or ID (shows all sessions if omitted)
        profile: Option<String>,
    },

    /// Show resource usage (CPU, memory, GPU) for session processes
    Top {
        /// Profile name or ID (shows all sessions if omitted)
        profile: Option<String>,
    },

    /// Show all sessions with details
    Status,

    /// Peek at recent terminal output
    Peek {
        /// Profile name, ID, or session name
        session: String,
        /// Tab names to show (all if omitted)
        #[arg(value_name = "TABS")]
        tabs: Vec<String>,
        /// Lines of output per tab
        #[arg(short = 'n', long, default_value = "50")]
        lines: u32,
    },

    /// Pin the current tab to the session's profile
    Pin,

    /// Unpin the current tab from the session's profile
    Unpin,

    /// Sync a window rename to the profile (called by tmux hook)
    #[command(hide = true)]
    SyncRename {
        /// Session name
        session: String,
        /// Window index
        window: u32,
        /// New window name
        name: String,
    },

    /// Handle pane death notification (called by tmux hook)
    #[command(name = "_pane-died", hide = true)]
    PaneDied {
        session_name: String,
        window_name: String,
        pane_id: String,
        exit_code: i32,
    },

    /// Handle bell notification (called by tmux hook)
    #[command(name = "_bell", hide = true)]
    Bell {
        session_name: String,
        window_name: String,
    },

    /// Profile management
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Notification management
    Notifications {
        #[command(subcommand)]
        action: NotificationAction,
    },

    /// Show or update settings
    Settings {
        /// Set terminal emulator (e.g. ghostty, alacritty, kitty, wezterm, terminal, iterm2)
        #[arg(long)]
        terminal: Option<String>,
        /// Set default shell
        #[arg(long)]
        shell: Option<String>,
        /// Set tmux binary path
        #[arg(long)]
        tmux_path: Option<String>,
    },
}

/// Profile subcommands.
#[derive(Subcommand)]
pub enum ProfileAction {
    /// List all profiles
    List,

    /// Delete a profile
    Delete {
        /// Profile name or ID
        id: String,
    },

    /// Save a new profile
    Save {
        /// Profile name
        name: String,
        /// Tab definition (`name:cwd[:command]`), repeatable
        #[arg(long)]
        tab: Vec<String>,
        /// Color (hex)
        #[arg(long, default_value = "#808080")]
        color: String,
    },

    /// Add a tab to an existing profile
    AddTab {
        /// Profile name or ID
        profile: String,
        /// Tab name
        #[arg(long)]
        name: String,
        /// Working directory
        #[arg(long)]
        cwd: String,
        /// Startup command
        #[arg(long)]
        command: Option<String>,
    },

    /// Show a profile's full definition
    Show {
        /// Profile name or ID
        id: String,
    },

    /// Edit a profile in $EDITOR
    Edit {
        /// Profile name or ID
        id: String,
    },

    /// Update profile fields inline
    Update {
        /// Profile name or ID
        id: String,
        /// New display name
        #[arg(long)]
        name: Option<String>,
        /// New color (hex or named)
        #[arg(long)]
        color: Option<String>,
    },

    /// Remove a tab from a profile
    RemoveTab {
        /// Profile name or ID
        profile: String,
        /// Tab name or 0-based index
        tab: String,
    },
}

/// Notification subcommands.
#[derive(Subcommand)]
pub enum NotificationAction {
    /// Install macOS notification app bundle
    Setup,
    /// Remove macOS notification app bundle
    Remove,
    /// Send a test notification to verify the notification system works
    Test,
}
