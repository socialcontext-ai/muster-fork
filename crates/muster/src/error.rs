//! Error types for the muster library.

use std::path::PathBuf;

/// Errors produced by the muster library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tmux not found in PATH")]
    TmuxNotFound,

    #[error("tmux command failed: {0}")]
    TmuxError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config directory error: {}", .0.display())]
    ConfigDir(PathBuf),

    #[error("profile not found: {0}")]
    ProfileNotFound(String),

    #[error("duplicate profile: {0}")]
    DuplicateProfile(String),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("invalid color: {0}")]
    InvalidColor(String),

    #[error("not inside a tmux session")]
    NotInTmux,

    #[error("not a muster-managed session")]
    NotMusterSession,
}

/// Convenience alias for `std::result::Result` with [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
