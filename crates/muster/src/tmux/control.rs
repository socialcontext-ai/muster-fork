use std::path::PathBuf;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::broadcast;

use crate::error::{Error, Result};

/// Events emitted from tmux control mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusterEvent {
    TabAdded {
        window_id: String,
    },
    TabClosed {
        window_id: String,
    },
    TabRenamed {
        window_id: String,
        name: String,
    },
    ActiveTabChanged {
        session_id: String,
        window_id: String,
    },
    SessionsChanged,
    SessionRenamed {
        name: String,
    },
    SessionEnded,
    LayoutChanged {
        window_id: String,
    },
    ClientDetached {
        client: String,
    },
    SubscriptionChanged {
        name: String,
        window_id: String,
        pane_id: String,
        value: String,
    },
}

/// A parsed line from the control mode stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlLine {
    Notification(MusterEvent),
    BeginResponse {
        timestamp: String,
        command_number: String,
        flags: String,
    },
    EndResponse {
        timestamp: String,
        command_number: String,
        flags: String,
    },
    ErrorResponse {
        timestamp: String,
        command_number: String,
        flags: String,
    },
    OutputLine(String),
    Unknown(String),
}

/// Parse a single line from the control mode stream.
pub fn parse_control_line(line: &str) -> ControlLine {
    if let Some(rest) = line.strip_prefix("%window-add ") {
        return ControlLine::Notification(MusterEvent::TabAdded {
            window_id: rest.trim().to_string(),
        });
    }
    if let Some(rest) = line.strip_prefix("%window-close ") {
        return ControlLine::Notification(MusterEvent::TabClosed {
            window_id: rest.trim().to_string(),
        });
    }
    if let Some(rest) = line.strip_prefix("%window-renamed ") {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() == 2 {
            return ControlLine::Notification(MusterEvent::TabRenamed {
                window_id: parts[0].to_string(),
                name: parts[1].to_string(),
            });
        }
    }
    if let Some(rest) = line.strip_prefix("%session-window-changed ") {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() == 2 {
            return ControlLine::Notification(MusterEvent::ActiveTabChanged {
                session_id: parts[0].to_string(),
                window_id: parts[1].to_string(),
            });
        }
    }
    if line == "%sessions-changed" {
        return ControlLine::Notification(MusterEvent::SessionsChanged);
    }
    if let Some(rest) = line.strip_prefix("%session-renamed ") {
        return ControlLine::Notification(MusterEvent::SessionRenamed {
            name: rest.trim().to_string(),
        });
    }
    if let Some(rest) = line.strip_prefix("%layout-change ") {
        // layout-change has multiple fields, we only need window_id
        let window_id = rest.split_whitespace().next().unwrap_or("").to_string();
        return ControlLine::Notification(MusterEvent::LayoutChanged { window_id });
    }
    if let Some(rest) = line.strip_prefix("%client-detached ") {
        return ControlLine::Notification(MusterEvent::ClientDetached {
            client: rest.trim().to_string(),
        });
    }
    if let Some(rest) = line.strip_prefix("%begin ") {
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() == 3 {
            return ControlLine::BeginResponse {
                timestamp: parts[0].to_string(),
                command_number: parts[1].to_string(),
                flags: parts[2].to_string(),
            };
        }
    }
    if let Some(rest) = line.strip_prefix("%end ") {
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() == 3 {
            return ControlLine::EndResponse {
                timestamp: parts[0].to_string(),
                command_number: parts[1].to_string(),
                flags: parts[2].to_string(),
            };
        }
    }
    if let Some(rest) = line.strip_prefix("%error ") {
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() == 3 {
            return ControlLine::ErrorResponse {
                timestamp: parts[0].to_string(),
                command_number: parts[1].to_string(),
                flags: parts[2].to_string(),
            };
        }
    }
    if let Some(rest) = line.strip_prefix("%subscription-changed ") {
        let parts: Vec<&str> = rest.splitn(4, ' ').collect();
        if parts.len() == 4 {
            return ControlLine::Notification(MusterEvent::SubscriptionChanged {
                name: parts[0].to_string(),
                window_id: parts[1].to_string(),
                pane_id: parts[2].to_string(),
                value: parts[3].to_string(),
            });
        }
    }
    // Lines inside a response block that aren't framing
    if line.starts_with('%') {
        return ControlLine::Unknown(line.to_string());
    }
    ControlLine::OutputLine(line.to_string())
}

/// Tracks parser state for the control mode stream.
#[derive(Debug, Default)]
pub struct StreamParser {
    in_response: bool,
    current_command: Option<String>,
    response_lines: Vec<String>,
}

impl StreamParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a line and get back any events that should be emitted.
    pub fn feed(&mut self, line: &str) -> Vec<MusterEvent> {
        let parsed = parse_control_line(line);
        let mut events = Vec::new();

        match parsed {
            ControlLine::Notification(event) => {
                events.push(event);
            }
            ControlLine::BeginResponse { command_number, .. } => {
                self.in_response = true;
                self.current_command = Some(command_number);
                self.response_lines.clear();
            }
            ControlLine::EndResponse { .. } | ControlLine::ErrorResponse { .. } => {
                self.in_response = false;
                self.current_command = None;
                self.response_lines.clear();
            }
            ControlLine::OutputLine(text) => {
                if self.in_response {
                    self.response_lines.push(text);
                }
                // Output lines outside a response are ignored (pane output, suppressed)
            }
            ControlLine::Unknown(_) => {}
        }

        events
    }
}

/// Control mode connection to a tmux session.
pub struct ControlMode {
    child: Child,
    tx: broadcast::Sender<MusterEvent>,
    tmux_path: PathBuf,
}

impl ControlMode {
    /// Start a control mode connection to a session.
    pub async fn connect(
        tmux_path: &std::path::Path,
        session: &str,
        tx: broadcast::Sender<MusterEvent>,
    ) -> Result<Self> {
        let mut child = Command::new(tmux_path)
            .args(["-C", "attach-session", "-t", session])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| Error::TmuxError(format!("failed to spawn control mode: {e}")))?;

        // Send refresh-client to suppress pane output
        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(b"refresh-client -f no-output\n")
                .await
                .map_err(|e| Error::TmuxError(format!("failed to write to control mode: {e}")))?;
        }

        Ok(Self {
            child,
            tx,
            tmux_path: tmux_path.to_path_buf(),
        })
    }

    /// Take ownership of the control mode's stdin for sending commands.
    /// Must be called before `spawn_reader()` (which consumes self).
    pub fn take_stdin(&mut self) -> Option<tokio::process::ChildStdin> {
        self.child.stdin.take()
    }

    /// Spawn a background task that reads the control mode stream and emits events.
    pub fn spawn_reader(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let Some(stdout) = self.child.stdout.take() else {
                return;
            };
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut parser = StreamParser::new();

            while let Ok(Some(line)) = lines.next_line().await {
                let events = parser.feed(&line);
                for event in events {
                    // If all receivers dropped, stop
                    if self.tx.send(event).is_err() {
                        return;
                    }
                }
            }
            // Stream ended — session died
            let _ = self.tx.send(MusterEvent::SessionEnded);
        })
    }

    /// Get the tmux path used by this connection (for reconnection).
    pub fn tmux_path(&self) -> &std::path::Path {
        &self.tmux_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_window_add() {
        let line = "%window-add @1";
        let parsed = parse_control_line(line);
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::TabAdded {
                window_id: "@1".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_window_close() {
        let line = "%window-close @1";
        let parsed = parse_control_line(line);
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::TabClosed {
                window_id: "@1".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_window_renamed() {
        let line = "%window-renamed @1 newname";
        let parsed = parse_control_line(line);
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::TabRenamed {
                window_id: "@1".to_string(),
                name: "newname".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_sessions_changed() {
        let parsed = parse_control_line("%sessions-changed");
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::SessionsChanged)
        );
    }

    #[test]
    fn test_parse_response_block() {
        let begin = "%begin 1234567890 1 0";
        let end = "%end 1234567890 1 0";

        match parse_control_line(begin) {
            ControlLine::BeginResponse { command_number, .. } => assert_eq!(command_number, "1"),
            other => panic!("expected BeginResponse, got {other:?}"),
        }
        match parse_control_line(end) {
            ControlLine::EndResponse { command_number, .. } => assert_eq!(command_number, "1"),
            other => panic!("expected EndResponse, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_mixed_stream() {
        let lines = vec![
            "%sessions-changed",
            "%begin 1234567890 1 0",
            "session1: 2 windows",
            "%end 1234567890 1 0",
            "%window-add @5",
            "%window-renamed @5 editor",
        ];

        let mut parser = StreamParser::new();
        let mut all_events = Vec::new();
        for line in lines {
            all_events.extend(parser.feed(line));
        }

        assert_eq!(all_events.len(), 3);
        assert_eq!(all_events[0], MusterEvent::SessionsChanged);
        assert_eq!(
            all_events[1],
            MusterEvent::TabAdded {
                window_id: "@5".to_string(),
            }
        );
        assert_eq!(
            all_events[2],
            MusterEvent::TabRenamed {
                window_id: "@5".to_string(),
                name: "editor".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_active_tab_changed() {
        let line = "%session-window-changed $1 @3";
        let parsed = parse_control_line(line);
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::ActiveTabChanged {
                session_id: "$1".to_string(),
                window_id: "@3".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_session_renamed() {
        let line = "%session-renamed newname";
        let parsed = parse_control_line(line);
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::SessionRenamed {
                name: "newname".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_client_detached() {
        let line = "%client-detached /dev/ttys001";
        let parsed = parse_control_line(line);
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::ClientDetached {
                client: "/dev/ttys001".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_subscription_changed() {
        let line = "%subscription-changed pd_5 @1 %5 1:0";
        let parsed = parse_control_line(line);
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::SubscriptionChanged {
                name: "pd_5".to_string(),
                window_id: "@1".to_string(),
                pane_id: "%5".to_string(),
                value: "1:0".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_subscription_changed_with_spaces_in_value() {
        let line = "%subscription-changed bell_1 @2 %10 1 extra data";
        let parsed = parse_control_line(line);
        assert_eq!(
            parsed,
            ControlLine::Notification(MusterEvent::SubscriptionChanged {
                name: "bell_1".to_string(),
                window_id: "@2".to_string(),
                pane_id: "%10".to_string(),
                value: "1 extra data".to_string(),
            })
        );
    }

    #[test]
    fn test_stream_parser_ignores_response_content() {
        let mut parser = StreamParser::new();

        // Response block should not emit events for output lines
        assert!(parser.feed("%begin 123 1 0").is_empty());
        assert!(parser.feed("some output data").is_empty());
        assert!(parser.feed("more output").is_empty());
        assert!(parser.feed("%end 123 1 0").is_empty());

        // But notifications around it should
        let events = parser.feed("%window-add @1");
        assert_eq!(events.len(), 1);
    }

    #[test]
    #[ignore]
    fn test_control_mode_receives_events() {
        // Verify that tmux -C control mode produces parseable events.
        // Uses a raw child process (not ControlMode) because -C mode
        // requires careful stdin lifetime management.
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let tmux_path = which::which("tmux").expect("tmux must be installed");
            let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());

            // Create a session
            let output = std::process::Command::new(&tmux_path)
                .args(["new-session", "-d", "-s", &session_name, "-n", "first"])
                .output()
                .expect("create session");
            assert!(output.status.success(), "failed to create test session");

            // Connect in control mode — keep stdin open to prevent %exit
            let mut child = tokio::process::Command::new(&tmux_path)
                .args(["-C", "attach-session", "-t", &session_name])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("spawn control mode");

            let stdout = child.stdout.take().unwrap();
            let mut stdin = child.stdin.take().unwrap();

            // Send a new-window command through control mode stdin
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(b"new-window -n second\n")
                .await
                .expect("write new-window");

            // Read lines and parse events
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut parser = StreamParser::new();
            let mut found = false;

            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
            let mut received = Vec::new();
            while let Ok(result) =
                tokio::time::timeout_at(deadline, lines.next_line()).await
            {
                let Some(line) = result.expect("read line") else {
                    break; // EOF
                };
                let events = parser.feed(&line);
                for event in events {
                    received.push(format!("{event:?}"));
                    if matches!(event, MusterEvent::TabAdded { .. } | MusterEvent::SessionsChanged) {
                        found = true;
                    }
                }
                if found {
                    break;
                }
            }
            assert!(found, "expected TabAdded or SessionsChanged, got: {received:?}");

            // Cleanup: drop stdin to let tmux exit, then kill session
            drop(stdin);
            drop(child);
            let _ = std::process::Command::new(&tmux_path)
                .args(["kill-session", "-t", &session_name])
                .output();
        });
    }
}
