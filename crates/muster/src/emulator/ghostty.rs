use std::process::Command;

use crate::error::{Error, Result};

use super::{Emulator, EmulatorHandle};

/// Ghostty terminal emulator implementation.
pub struct GhosttyEmulator;

impl GhosttyEmulator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GhosttyEmulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Emulator for GhosttyEmulator {
    fn launch(&self, session_name: &str) -> Result<EmulatorHandle> {
        let child = Command::new("open")
            .args([
                "-na",
                "Ghostty.app",
                "--args",
                "-e",
                "tmux",
                "attach",
                "-t",
                session_name,
            ])
            .spawn()
            .map_err(|e| Error::TmuxError(format!("failed to launch Ghostty: {e}")))?;

        Ok(EmulatorHandle {
            pid: Some(child.id()),
        })
    }

    fn is_running(&self, session_name: &str) -> Result<bool> {
        let output = Command::new("ps")
            .args(["aux"])
            .output()
            .map_err(|e| Error::TmuxError(format!("failed to run ps: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Look for a Ghostty process with the session name in its args
        Ok(stdout.lines().any(|line| {
            line.contains("Ghostty") && line.contains("tmux") && line.contains(session_name)
        }))
    }

    fn attach_command(&self, session_name: &str) -> Vec<String> {
        vec![
            "open".to_string(),
            "-na".to_string(),
            "Ghostty.app".to_string(),
            "--args".to_string(),
            "-e".to_string(),
            "tmux".to_string(),
            "attach".to_string(),
            "-t".to_string(),
            session_name.to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ghostty_attach_command() {
        let emu = GhosttyEmulator::new();
        let cmd = emu.attach_command("muster_test123");

        assert_eq!(cmd[0], "open");
        assert_eq!(cmd[1], "-na");
        assert_eq!(cmd[2], "Ghostty.app");
        assert!(cmd.contains(&"tmux".to_string()));
        assert!(cmd.contains(&"attach".to_string()));
        assert!(cmd.contains(&"muster_test123".to_string()));
    }
}
