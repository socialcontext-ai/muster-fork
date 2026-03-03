//! Muster — terminal session group management built on tmux.
//!
//! This library provides Rust bindings for tmux command execution, control mode
//! event streaming, profile management, session lifecycle, and runtime theming.

pub mod config;
pub mod emulator;
pub mod error;
mod muster;
pub mod session;
pub mod tmux;

pub use config::{Profile, ProfileStore, Settings, SettingsStore, TabProfile};
pub use emulator::{Emulator, EmulatorHandle, GhosttyEmulator};
pub use error::{Error, Result};
pub use muster::Muster;
pub use tmux::{
    ControlMode, MusterEvent, SessionInfo, StreamParser, TmuxClient, TmuxSession, TmuxWindow,
};
