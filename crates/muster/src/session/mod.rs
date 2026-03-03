pub mod theme;

use crate::config::profile::{Profile, TabProfile};
use crate::error::Result;
use crate::tmux::client::{TmuxClient, SESSION_PREFIX};
use crate::tmux::types::{SessionInfo, TmuxWindow};

/// Resolve the shell to use for new tmux panes.
///
/// Priority: explicit shell setting → `$SHELL` env var → None (tmux default).
pub fn resolve_shell(shell_setting: Option<&str>) -> Option<String> {
    if let Some(sh) = shell_setting {
        if !sh.is_empty() {
            return Some(sh.to_string());
        }
    }
    std::env::var("SHELL").ok().filter(|s| !s.is_empty())
}

/// Create a tmux session from a profile.
///
/// Steps:
/// 1. Create detached session with first tab
/// 2. Set `default-command` so manually-created panes use the right shell
/// 3. Create additional windows for remaining tabs
/// 4. Send startup commands to tabs that have them
/// 5. Set `@muster_*` metadata
pub fn create_from_profile(
    client: &TmuxClient,
    profile: &Profile,
    shell: Option<&str>,
) -> Result<SessionInfo> {
    let session_name = format!("{SESSION_PREFIX}{}", profile.id);

    let first_tab = profile.tabs.first().cloned().unwrap_or_else(|| TabProfile {
        name: "Shell".to_string(),
        cwd: "/tmp".to_string(),
        command: None,
    });

    // tmux sets $SHELL in pane processes from the global `default-shell` option.
    // Temporarily set it to the resolved shell so the session gets the right value.
    let prev_default_shell = if let Some(sh) = shell {
        let prev = client.get_global_option("default-shell")?;
        client.set_global_option("default-shell", sh)?;
        prev
    } else {
        None
    };

    // Create the session with the first window
    let create_result = client.new_session(&session_name, &first_tab.name, &first_tab.cwd, shell);

    // Restore default-shell immediately
    if let Some(ref prev) = prev_default_shell {
        client.set_global_option("default-shell", prev)?;
    }

    create_result?;

    // Set default-command so manually-created panes also use the right shell
    if let Some(sh) = shell {
        client.set_option(&session_name, "default-command", sh)?;
    }

    // Send startup command for first tab if specified
    if let Some(ref cmd) = first_tab.command {
        client.send_keys(&session_name, 0, cmd)?;
    }

    // Create additional windows
    for (i, tab) in profile.tabs.iter().enumerate().skip(1) {
        client.new_window(&session_name, &tab.name, &tab.cwd, shell)?;
        if let Some(ref cmd) = tab.command {
            let index = u32::try_from(i).unwrap_or(0);
            client.send_keys(&session_name, index, cmd)?;
        }
    }

    // Set metadata
    client.set_session_metadata(
        &session_name,
        &profile.name,
        &profile.color,
        Some(&profile.id),
    )?;

    // Return session info
    let windows = client.list_windows(&session_name)?;
    Ok(SessionInfo {
        session_name,
        display_name: profile.name.clone(),
        color: profile.color.clone(),
        profile_id: Some(profile.id.clone()),
        window_count: u32::try_from(windows.len()).unwrap_or(0),
        attached: false,
    })
}

/// Destroy a tmux session.
pub fn destroy(client: &TmuxClient, session_name: &str) -> Result<()> {
    client.kill_session(session_name)
}

/// Get windows for a session.
pub fn get_windows(client: &TmuxClient, session_name: &str) -> Result<Vec<TmuxWindow>> {
    client.list_windows(session_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile() -> Profile {
        Profile {
            id: format!("test_{}", uuid::Uuid::new_v4()),
            name: "Test Project".to_string(),
            color: "#f97316".to_string(),
            tabs: vec![
                TabProfile {
                    name: "Shell".to_string(),
                    cwd: "/tmp".to_string(),
                    command: None,
                },
                TabProfile {
                    name: "Server".to_string(),
                    cwd: "/tmp".to_string(),
                    command: Some("echo hello".to_string()),
                },
            ],
        }
    }

    #[test]
    #[ignore]
    fn test_create_session_from_profile() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let profile = test_profile();
        let session_name = format!("{SESSION_PREFIX}{}", profile.id);

        let info = create_from_profile(&client, &profile, None).expect("create session");
        assert_eq!(info.display_name, "Test Project");
        assert_eq!(info.window_count, 2);

        // Verify windows exist
        let windows = client.list_windows(&session_name).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].name, "Shell");
        assert_eq!(windows[1].name, "Server");

        // Cleanup
        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_create_session_sets_metadata() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let profile = test_profile();
        let session_name = format!("{SESSION_PREFIX}{}", profile.id);

        create_from_profile(&client, &profile, None).expect("create session");

        let name = client.get_option(&session_name, "@muster_name").unwrap();
        let color = client.get_option(&session_name, "@muster_color").unwrap();
        let pid = client.get_option(&session_name, "@muster_profile").unwrap();

        assert_eq!(name, Some("Test Project".to_string()));
        assert_eq!(color, Some("#f97316".to_string()));
        assert_eq!(pid, Some(profile.id.clone()));

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_destroy_session() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let profile = test_profile();
        let session_name = format!("{SESSION_PREFIX}{}", profile.id);

        create_from_profile(&client, &profile, None).expect("create session");
        assert!(client.has_session(&session_name).unwrap());

        destroy(&client, &session_name).expect("destroy session");
        assert!(!client.has_session(&session_name).unwrap());
    }

    #[test]
    #[ignore]
    fn test_create_with_startup_commands() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let profile = test_profile();
        let session_name = format!("{SESSION_PREFIX}{}", profile.id);

        // The profile has "echo hello" as a startup command on the second tab.
        // We can't easily verify the command ran, but we can verify the session
        // was created without errors (send-keys doesn't fail if the command is invalid).
        create_from_profile(&client, &profile, None).expect("create session");
        assert!(client.has_session(&session_name).unwrap());

        client.kill_session(&session_name).ok();
    }
}
