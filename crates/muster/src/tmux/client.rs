use std::path::PathBuf;
use std::process::Command;

use crate::error::{Error, Result};
use crate::tmux::types::{TmuxSession, TmuxWindow};

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
    pub fn new_session(&self, name: &str, first_window_name: &str, cwd: &str) -> Result<()> {
        self.cmd(&[
            "new-session",
            "-d",
            "-s",
            name,
            "-n",
            first_window_name,
            "-c",
            cwd,
        ])?;
        Ok(())
    }

    /// Kill (destroy) a session.
    pub fn kill_session(&self, name: &str) -> Result<()> {
        self.cmd(&["kill-session", "-t", name])?;
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

    // ---- Integration tests (need real tmux) ----

    #[test]
    #[ignore]
    fn test_create_and_destroy_session() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());

        // Create
        client
            .new_session(&session_name, "shell", "/tmp")
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
            .new_session(&managed_name, "shell", "/tmp")
            .expect("create managed");
        client
            .new_session(&unmanaged_name, "shell", "/tmp")
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
            .new_session(&session_name, "first", "/tmp")
            .expect("create session");

        let windows = client.list_windows(&session_name).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].name, "first");
        assert_eq!(windows[0].index, 0);

        client.kill_session(&session_name).ok();
    }
}
