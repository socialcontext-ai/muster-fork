//! Muster — terminal session group management built on tmux.
//!
//! This library provides Rust bindings for tmux command execution, control mode
//! event streaming, profile management, session lifecycle, and runtime theming.

pub mod error;
pub mod tmux;

pub use error::{Error, Result};
pub use tmux::{TmuxClient, TmuxSession, TmuxWindow};
