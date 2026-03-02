/// A tmux session as returned by `list-sessions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxSession {
    pub name: String,
    pub windows: u32,
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
