//! Muster — terminal session group management built on tmux.
//!
//! This library provides Rust bindings for tmux command execution, control mode
//! event streaming, profile management, session lifecycle, and runtime theming.

pub mod config;
pub mod error;
mod muster;
pub mod session;
pub mod tmux;

pub use config::{PaneProfile, Profile, ProfileStore, Settings, SettingsStore, TabProfile};
pub use error::{Error, Result};
pub use muster::{Muster, PinResult};
pub use tmux::{
    ControlMode, MusterEvent, PaneContext, SessionInfo, StreamParser, TmuxClient, TmuxPane,
    TmuxSession, TmuxWindow,
};
