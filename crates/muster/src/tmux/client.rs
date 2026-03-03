use std::path::PathBuf;
use std::process::Command;

use crate::error::{Error, Result};
use crate::tmux::types::{SessionInfo, TmuxSession, TmuxWindow};

/// Prefix for all muster-managed tmux sessions.
pub const SESSION_PREFIX: &str = "muster_";

/// Client for executing tmux commands and parsing output.
pub struct TmuxClient {
    tmux_path: PathBuf,
}

impl TmuxClient {
    /// Create a new client, discovering tmux in PATH.
    pub fn new() -> Result<Self> {
        let path = which::which("tmux").map_err(|_| Error::TmuxNotFound)?;
        Ok(Self { tmux_path: path })
    }

    /// Create a client with an explicit tmux path.
    pub fn with_path(path: PathBuf) -> Self {
        Self { tmux_path: path }
    }

    /// Execute a tmux command, returning stdout on success.
    pub fn cmd(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.tmux_path)
            .args(args)
            .output()
            .map_err(|e| Error::TmuxError(format!("failed to spawn tmux: {e}")))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // "no server running" is not an error for queries — just means no sessions
            if stderr.contains("no server running") || stderr.contains("no current session") {
                return Ok(String::new());
            }
            Err(Error::TmuxError(stderr.into_owned()))
        }
    }

    /// Build the argument list for a tmux command. Exposed for testing.
    pub fn build_args<'a>(command: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
        let mut args = vec![command];
        args.extend_from_slice(extra);
        args
    }

    /// Create a new detached session.
    ///
    /// Propagates the current process environment to the initial pane via `-e` flags,
    /// since tmux otherwise inherits from the (potentially stale) server environment.
    /// If `shell` is provided, it is used as the shell command for the initial window.
    pub fn new_session(
        &self,
        name: &str,
        first_window_name: &str,
        cwd: &str,
        shell: Option<&str>,
    ) -> Result<()> {
        let env_pairs: Vec<String> = std::env::vars()
            .filter(|(k, _)| !k.starts_with("TMUX") && k != "TERM")
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let mut args = vec!["new-session", "-d", "-s", name, "-n", first_window_name, "-c", cwd];
        for pair in &env_pairs {
            args.push("-e");
            args.push(pair);
        }
        if let Some(sh) = shell {
            args.push(sh);
        }
        self.cmd(&args)?;
        Ok(())
    }

    /// Kill (destroy) a session.
    pub fn kill_session(&self, name: &str) -> Result<()> {
        self.cmd(&["kill-session", "-t", name])?;
        Ok(())
    }

    /// Create a new window in a session.
    ///
    /// If `shell` is provided, it is used as the shell command for the window.
    pub fn new_window(
        &self,
        session: &str,
        name: &str,
        cwd: &str,
        shell: Option<&str>,
    ) -> Result<()> {
        let mut args = vec!["new-window", "-t", session, "-n", name, "-c", cwd];
        if let Some(sh) = shell {
            args.push(sh);
        }
        self.cmd(&args)?;
        Ok(())
    }

    /// Send keys (a command) to a specific window in a session.
    pub fn send_keys(&self, session: &str, window_index: u32, keys: &str) -> Result<()> {
        let target = format!("{session}:{window_index}");
        self.cmd(&["send-keys", "-t", &target, keys, "Enter"])?;
        Ok(())
    }

    /// Kill (close) a window in a session.
    pub fn kill_window(&self, session: &str, window_index: u32) -> Result<()> {
        let target = format!("{session}:{window_index}");
        self.cmd(&["kill-window", "-t", &target])?;
        Ok(())
    }

    /// Select (switch to) a window in a session.
    pub fn select_window(&self, session: &str, window_index: u32) -> Result<()> {
        let target = format!("{session}:{window_index}");
        self.cmd(&["select-window", "-t", &target])?;
        Ok(())
    }

    /// Rename a window in a session.
    pub fn rename_window(&self, session: &str, window_index: u32, new_name: &str) -> Result<()> {
        let target = format!("{session}:{window_index}");
        self.cmd(&["rename-window", "-t", &target, new_name])?;
        Ok(())
    }

    /// Check if a session exists.
    pub fn has_session(&self, name: &str) -> Result<bool> {
        let output = Command::new(&self.tmux_path)
            .args(["has-session", "-t", name])
            .output()
            .map_err(|e| Error::TmuxError(format!("failed to spawn tmux: {e}")))?;
        Ok(output.status.success())
    }

    /// List all sessions, parsing structured output.
    pub fn list_sessions(&self) -> Result<Vec<TmuxSession>> {
        let output = self.cmd(&[
            "list-sessions",
            "-F",
            "#{session_name}\t#{session_windows}\t#{session_attached}",
        ])?;
        Ok(Self::parse_session_list(&output))
    }

    /// List only muster-managed sessions (those with the `muster_` prefix).
    pub fn list_managed_sessions(&self) -> Result<Vec<TmuxSession>> {
        let sessions = self.list_sessions()?;
        Ok(sessions
            .into_iter()
            .filter(|s| s.name.starts_with(SESSION_PREFIX))
            .collect())
    }

    /// Parse `list-sessions -F` output into structured data.
    pub fn parse_session_list(output: &str) -> Vec<TmuxSession> {
        output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 3 {
                    return None;
                }
                Some(TmuxSession {
                    name: parts[0].to_string(),
                    windows: parts[1].parse().unwrap_or(0),
                    attached: parts[2] != "0",
                })
            })
            .collect()
    }

    /// List windows for a session.
    pub fn list_windows(&self, session: &str) -> Result<Vec<TmuxWindow>> {
        let output = self.cmd(&[
            "list-windows",
            "-t",
            session,
            "-F",
            "#{window_index}\t#{window_name}\t#{pane_current_path}\t#{window_active}",
        ])?;
        Ok(Self::parse_window_list(&output))
    }

    // ---- User option (metadata) methods ----

    /// Set a tmux session environment variable.
    pub fn set_environment(&self, session: &str, name: &str, value: &str) -> Result<()> {
        self.cmd(&["set-environment", "-t", session, name, value])?;
        Ok(())
    }

    /// Set a tmux user option on a session.
    pub fn set_option(&self, session: &str, key: &str, value: &str) -> Result<()> {
        self.cmd(&["set-option", "-t", session, key, value])?;
        Ok(())
    }

    /// Get a tmux user option from a session. Returns None if the option is not set.
    pub fn get_option(&self, session: &str, key: &str) -> Result<Option<String>> {
        let output = Command::new(&self.tmux_path)
            .args(["show-option", "-t", session, "-v", key])
            .output()
            .map_err(|e| Error::TmuxError(format!("failed to spawn tmux: {e}")))?;

        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        } else {
            // Option not set — not an error
            Ok(None)
        }
    }

    /// Set all muster metadata options on a session.
    pub fn set_session_metadata(
        &self,
        session: &str,
        name: &str,
        color: &str,
        profile_id: Option<&str>,
    ) -> Result<()> {
        self.set_option(session, "@muster_name", name)?;
        self.set_option(session, "@muster_color", color)?;
        if let Some(pid) = profile_id {
            self.set_option(session, "@muster_profile", pid)?;
        }
        Ok(())
    }

    /// List managed sessions with their @muster_* metadata.
    pub fn list_sessions_with_metadata(&self) -> Result<Vec<SessionInfo>> {
        let format = [
            "#{session_name}",
            "#{session_windows}",
            "#{session_attached}",
            "#{@muster_name}",
            "#{@muster_color}",
            "#{@muster_profile}",
        ]
        .join("\t");
        let output = self.cmd(&["list-sessions", "-F", &format])?;
        Ok(Self::parse_session_info_list(&output))
    }

    /// Parse list-sessions output that includes @muster_* metadata.
    pub fn parse_session_info_list(output: &str) -> Vec<SessionInfo> {
        output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 6 {
                    return None;
                }
                let session_name = parts[0];
                if !session_name.starts_with(SESSION_PREFIX) {
                    return None;
                }
                let display_name = if parts[3].is_empty() {
                    session_name
                        .strip_prefix(SESSION_PREFIX)
                        .unwrap_or(session_name)
                        .to_string()
                } else {
                    parts[3].to_string()
                };
                let color = if parts[4].is_empty() {
                    "#808080".to_string()
                } else {
                    parts[4].to_string()
                };
                let profile_id = if parts[5].is_empty() {
                    None
                } else {
                    Some(parts[5].to_string())
                };
                Some(SessionInfo {
                    session_name: session_name.to_string(),
                    display_name,
                    color,
                    profile_id,
                    window_count: parts[1].parse().unwrap_or(0),
                    attached: parts[2] != "0",
                })
            })
            .collect()
    }

    /// Parse `list-windows -F` output into structured data.
    pub fn parse_window_list(output: &str) -> Vec<TmuxWindow> {
        output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 4 {
                    return None;
                }
                Some(TmuxWindow {
                    index: parts[0].parse().unwrap_or(0),
                    name: parts[1].to_string(),
                    cwd: parts[2].to_string(),
                    active: parts[3] != "0",
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Unit tests (no tmux needed) ----

    #[test]
    fn test_build_command() {
        let args = TmuxClient::build_args("new-session", &["-d", "-s", "test"]);
        assert_eq!(args, vec!["new-session", "-d", "-s", "test"]);

        let args = TmuxClient::build_args("list-sessions", &["-F", "#{session_name}"]);
        assert_eq!(args, vec!["list-sessions", "-F", "#{session_name}"]);
    }

    #[test]
    fn test_parse_session_list() {
        let output = "muster_abc123\t3\t1\npersonal\t1\t0\nmuster_def456\t2\t0\n";
        let sessions = TmuxClient::parse_session_list(output);

        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0].name, "muster_abc123");
        assert_eq!(sessions[0].windows, 3);
        assert!(sessions[0].attached);
        assert_eq!(sessions[1].name, "personal");
        assert_eq!(sessions[1].windows, 1);
        assert!(!sessions[1].attached);
        assert_eq!(sessions[2].name, "muster_def456");
        assert_eq!(sessions[2].windows, 2);
        assert!(!sessions[2].attached);
    }

    #[test]
    fn test_parse_session_list_empty() {
        let sessions = TmuxClient::parse_session_list("");
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_parse_window_list() {
        let output = "0\tShell\t/Users/sbb/work\t1\n1\tServer\t/Users/sbb/work/app\t0\n";
        let windows = TmuxClient::parse_window_list(output);

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].index, 0);
        assert_eq!(windows[0].name, "Shell");
        assert_eq!(windows[0].cwd, "/Users/sbb/work");
        assert!(windows[0].active);
        assert_eq!(windows[1].index, 1);
        assert_eq!(windows[1].name, "Server");
        assert_eq!(windows[1].cwd, "/Users/sbb/work/app");
        assert!(!windows[1].active);
    }

    #[test]
    fn test_parse_window_list_empty() {
        let windows = TmuxClient::parse_window_list("");
        assert!(windows.is_empty());
    }

    #[test]
    fn test_session_prefix_filter() {
        let output = "muster_abc123\t3\t0\npersonal\t1\t0\nmuster_def456\t2\t0\n";
        let all = TmuxClient::parse_session_list(output);
        let managed: Vec<_> = all
            .into_iter()
            .filter(|s| s.name.starts_with(SESSION_PREFIX))
            .collect();

        assert_eq!(managed.len(), 2);
        assert_eq!(managed[0].name, "muster_abc123");
        assert_eq!(managed[1].name, "muster_def456");
    }

    #[test]
    fn test_parse_session_info_with_metadata() {
        let output =
            "muster_abc123\t3\t1\tPKM Project\t#f97316\tprofile_abc123\npersonal\t1\t0\t\t\t\n";
        let sessions = TmuxClient::parse_session_info_list(output);

        // Only muster_ sessions are included
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_name, "muster_abc123");
        assert_eq!(sessions[0].display_name, "PKM Project");
        assert_eq!(sessions[0].color, "#f97316");
        assert_eq!(sessions[0].profile_id, Some("profile_abc123".to_string()));
        assert_eq!(sessions[0].window_count, 3);
        assert!(sessions[0].attached);
    }

    #[test]
    fn test_parse_session_info_without_metadata() {
        let output = "muster_orphan\t1\t0\t\t\t\n";
        let sessions = TmuxClient::parse_session_info_list(output);

        assert_eq!(sessions.len(), 1);
        // Defaults: name derived from session, default color, no profile
        assert_eq!(sessions[0].display_name, "orphan");
        assert_eq!(sessions[0].color, "#808080");
        assert_eq!(sessions[0].profile_id, None);
    }

    // ---- Integration tests (need real tmux) ----

    #[test]
    #[ignore]
    fn test_create_and_destroy_session() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());

        // Create
        client
            .new_session(&session_name, "shell", "/tmp", None)
            .expect("create session");
        assert!(client.has_session(&session_name).unwrap());

        // Verify in list
        let sessions = client.list_sessions().unwrap();
        assert!(sessions.iter().any(|s| s.name == session_name));

        // Destroy
        client.kill_session(&session_name).expect("kill session");
        assert!(!client.has_session(&session_name).unwrap());
    }

    #[test]
    #[ignore]
    fn test_list_sessions_filters_prefix() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let managed_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        let unmanaged_name = format!("personal_test_{}", uuid::Uuid::new_v4());

        client
            .new_session(&managed_name, "shell", "/tmp", None)
            .expect("create managed");
        client
            .new_session(&unmanaged_name, "shell", "/tmp", None)
            .expect("create unmanaged");

        let managed = client.list_managed_sessions().unwrap();
        assert!(managed.iter().any(|s| s.name == managed_name));
        assert!(!managed.iter().any(|s| s.name == unmanaged_name));

        // Cleanup
        client.kill_session(&managed_name).ok();
        client.kill_session(&unmanaged_name).ok();
    }

    #[test]
    #[ignore]
    fn test_list_windows() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());

        client
            .new_session(&session_name, "first", "/tmp", None)
            .expect("create session");

        let windows = client.list_windows(&session_name).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].name, "first");
        assert_eq!(windows[0].index, 0);

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_set_and_get_user_option() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "shell", "/tmp", None)
            .expect("create session");

        client
            .set_option(&session_name, "@muster_name", "Test Project")
            .expect("set option");
        let value = client
            .get_option(&session_name, "@muster_name")
            .expect("get option");
        assert_eq!(value, Some("Test Project".to_string()));

        // Non-existent option returns None
        let missing = client
            .get_option(&session_name, "@muster_nonexistent")
            .expect("get missing option");
        assert!(missing.is_none());

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_session_with_metadata() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "shell", "/tmp", None)
            .expect("create session");

        client
            .set_session_metadata(&session_name, "My Project", "#ff6600", Some("profile_123"))
            .expect("set metadata");

        let name = client.get_option(&session_name, "@muster_name").unwrap();
        let color = client.get_option(&session_name, "@muster_color").unwrap();
        let profile = client.get_option(&session_name, "@muster_profile").unwrap();

        assert_eq!(name, Some("My Project".to_string()));
        assert_eq!(color, Some("#ff6600".to_string()));
        assert_eq!(profile, Some("profile_123".to_string()));

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_list_sessions_with_metadata() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "shell", "/tmp", None)
            .expect("create session");

        client
            .set_session_metadata(&session_name, "Listed Project", "#00ff00", None)
            .expect("set metadata");

        let sessions = client
            .list_sessions_with_metadata()
            .expect("list with metadata");
        let found = sessions.iter().find(|s| s.session_name == session_name);
        assert!(found.is_some());
        let s = found.unwrap();
        assert_eq!(s.display_name, "Listed Project");
        assert_eq!(s.color, "#00ff00");
        assert!(s.profile_id.is_none());

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_add_window() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "first", "/tmp", None)
            .expect("create session");

        client
            .new_window(&session_name, "second", "/tmp", None)
            .expect("add window");

        let windows = client.list_windows(&session_name).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[1].name, "second");

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_close_window() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "first", "/tmp", None)
            .expect("create session");
        client
            .new_window(&session_name, "second", "/tmp", None)
            .expect("add window");

        // Close second window
        client.kill_window(&session_name, 1).expect("close window");

        let windows = client.list_windows(&session_name).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].name, "first");

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_switch_window() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "first", "/tmp", None)
            .expect("create session");
        client
            .new_window(&session_name, "second", "/tmp", None)
            .expect("add window");

        // Switch back to first window
        client
            .select_window(&session_name, 0)
            .expect("switch window");

        let windows = client.list_windows(&session_name).unwrap();
        let active = windows.iter().find(|w| w.active).unwrap();
        assert_eq!(active.index, 0);

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_rename_window() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "original", "/tmp", None)
            .expect("create session");

        client
            .rename_window(&session_name, 0, "renamed")
            .expect("rename window");

        let windows = client.list_windows(&session_name).unwrap();
        assert_eq!(windows[0].name, "renamed");

        client.kill_session(&session_name).ok();
    }
}
