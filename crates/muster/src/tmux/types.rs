/// A tmux session as returned by `list-sessions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxSession {
    pub name: String,
    pub windows: u32,
    pub attached: bool,
}

/// Application-level session info including @muster_* user option metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub session_name: String,
    pub display_name: String,
    pub color: String,
    pub profile_id: Option<String>,
    pub window_count: u32,
    pub attached: bool,
}

/// A tmux window as returned by `list-windows`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxWindow {
    pub index: u32,
    pub name: String,
    pub cwd: String,
    pub active: bool,
}
