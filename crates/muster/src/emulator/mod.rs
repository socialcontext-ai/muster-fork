pub mod ghostty;

use crate::error::Result;

/// Handle for a spawned emulator process.
pub struct EmulatorHandle {
    pub pid: Option<u32>,
}

/// Trait for terminal emulator implementations.
pub trait Emulator: Send + Sync {
    /// Launch the emulator attached to a tmux session.
    fn launch(&self, session_name: &str) -> Result<EmulatorHandle>;

    /// Check if an emulator window is already open for this session.
    fn is_running(&self, session_name: &str) -> Result<bool>;

    /// Return the command and args needed to attach to a session.
    fn attach_command(&self, session_name: &str) -> Vec<String>;
}

pub use ghostty::GhosttyEmulator;

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock emulator for testing.
    struct MockEmulator {
        running_sessions: Vec<String>,
    }

    impl MockEmulator {
        fn new(sessions: Vec<String>) -> Self {
            Self {
                running_sessions: sessions,
            }
        }
    }

    impl Emulator for MockEmulator {
        fn launch(&self, _session_name: &str) -> Result<EmulatorHandle> {
            Ok(EmulatorHandle { pid: Some(12345) })
        }

        fn is_running(&self, session_name: &str) -> Result<bool> {
            Ok(self.running_sessions.contains(&session_name.to_string()))
        }

        fn attach_command(&self, session_name: &str) -> Vec<String> {
            vec![
                "mock-term".to_string(),
                "-e".to_string(),
                "tmux".to_string(),
                "attach".to_string(),
                "-t".to_string(),
                session_name.to_string(),
            ]
        }
    }

    #[test]
    fn test_emulator_trait_mock() {
        let mock = MockEmulator::new(vec!["muster_test".to_string()]);

        let handle = mock.launch("muster_test").unwrap();
        assert_eq!(handle.pid, Some(12345));

        assert!(mock.is_running("muster_test").unwrap());
        assert!(!mock.is_running("muster_other").unwrap());

        let cmd = mock.attach_command("muster_test");
        assert_eq!(cmd[0], "mock-term");
        assert!(cmd.contains(&"muster_test".to_string()));
    }
}
