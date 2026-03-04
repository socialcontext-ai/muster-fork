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

/// Set up panes within a window based on the tab profile.
///
/// If the tab has panes defined, creates splits and applies the layout.
/// Otherwise, falls back to single-pane behavior (send tab-level command).
fn setup_window_panes(
    client: &TmuxClient,
    session_name: &str,
    window_index: u32,
    tab: &TabProfile,
    shell: Option<&str>,
) -> Result<()> {
    if tab.panes.is_empty() {
        // Single-pane behavior: send tab-level command
        if let Some(ref cmd) = tab.command {
            client.send_keys(session_name, window_index, cmd)?;
        }
        return Ok(());
    }

    // Multi-pane: create splits for panes after the first
    for pane in tab.panes.iter().skip(1) {
        let cwd = pane.cwd.as_deref().unwrap_or(&tab.cwd);
        client.split_window(session_name, window_index, cwd, shell)?;
    }

    // Apply layout (must happen after all splits are created).
    // Best-effort: a bad or stale layout string shouldn't prevent session creation.
    if let Some(ref layout) = tab.layout {
        if let Err(e) = client.select_layout(session_name, window_index, layout) {
            tracing::warn!(window_index, %e, "failed to apply saved layout, using default");
        }
    }

    // Send commands to each pane
    for (pane_idx, pane) in tab.panes.iter().enumerate() {
        let idx = u32::try_from(pane_idx).unwrap_or(0);

        // If pane 0 has a different cwd than the tab, cd into it
        if pane_idx == 0 {
            if let Some(ref pane_cwd) = pane.cwd {
                if pane_cwd != &tab.cwd {
                    client.send_keys_to_pane(
                        session_name,
                        window_index,
                        idx,
                        &format!("cd {pane_cwd}"),
                    )?;
                }
            }
        }

        if let Some(ref cmd) = pane.command {
            client.send_keys_to_pane(session_name, window_index, idx, cmd)?;
        }
    }

    Ok(())
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
        layout: None,
        panes: vec![],
    });

    // Create the session with the first window
    client.new_session(&session_name, &first_tab.name, &first_tab.cwd, shell)?;

    // Set default-command so panes the user creates manually also use the right shell
    if let Some(sh) = shell {
        client.set_option(&session_name, "default-command", sh)?;
    }

    // Set up first window (panes or single-pane)
    setup_window_panes(client, &session_name, 0, &first_tab, shell)?;

    // Create additional windows
    for (i, tab) in profile.tabs.iter().enumerate().skip(1) {
        let index = u32::try_from(i).unwrap_or(0);
        client.new_window(&session_name, &tab.name, &tab.cwd, shell)?;
        setup_window_panes(client, &session_name, index, tab, shell)?;
    }

    // Set metadata
    client.set_session_metadata(
        &session_name,
        &profile.name,
        &profile.color,
        Some(&profile.id),
    )?;

    // Mark all profile-created windows as pinned (before apply_theme sees them)
    let windows = client.list_windows(&session_name)?;
    for win in &windows {
        client.set_window_option(&session_name, win.index, "@muster_pinned", "1")?;
        client.set_window_option(&session_name, win.index, "@muster_tab_name", &win.name)?;
    }

    // Set up notification hooks (pane-died, alert-bell)
    setup_hooks(client, &session_name)?;

    // Return session info
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

/// Set up tmux hooks for session notifications.
///
/// Installs `pane-died` and `alert-bell` hooks that invoke one-shot `muster`
/// subcommands to deliver desktop notifications. Also sets `remain-on-exit on`
/// so the `pane-died` hook fires before the pane is destroyed.
fn setup_hooks(client: &TmuxClient, session_name: &str) -> Result<()> {
    client.cmd(&["set-option", "-t", session_name, "remain-on-exit", "on"])?;

    let pane_died_hook = concat!(
        "run-shell -b \"muster _pane-died",
        " #{session_name} '#{window_name}' #{pane_id} #{pane_dead_status}\""
    );
    client.cmd(&["set-hook", "-t", session_name, "pane-died", pane_died_hook])?;

    let bell_hook = concat!(
        "run-shell -b \"muster _bell",
        " #{session_name} '#{window_name}'\""
    );
    client.cmd(&["set-hook", "-t", session_name, "alert-bell", bell_hook])?;

    Ok(())
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
                    layout: None,
                    panes: vec![],
                },
                TabProfile {
                    name: "Server".to_string(),
                    cwd: "/tmp".to_string(),
                    command: Some("echo hello".to_string()),
                    layout: None,
                    panes: vec![],
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

    #[test]
    #[ignore]
    fn test_create_session_sets_remain_on_exit() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let profile = test_profile();
        let session_name = format!("{SESSION_PREFIX}{}", profile.id);

        create_from_profile(&client, &profile, None).expect("create session");

        let output = client
            .cmd(&["show-option", "-t", &session_name, "-v", "remain-on-exit"])
            .unwrap();
        assert_eq!(output.trim(), "on");

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_create_session_sets_alert_bell_hook() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let profile = test_profile();
        let session_name = format!("{SESSION_PREFIX}{}", profile.id);

        create_from_profile(&client, &profile, None).expect("create session");

        // alert-bell is a session-level hook
        let hooks = client
            .cmd(&["show-hooks", "-t", &session_name])
            .unwrap();
        assert!(
            hooks.contains("alert-bell"),
            "alert-bell hook not found in: {hooks}"
        );
        assert!(
            hooks.contains("muster _bell"),
            "hook should invoke muster _bell: {hooks}"
        );

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_create_session_sets_pane_died_hook() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let profile = test_profile();
        let session_name = format!("{SESSION_PREFIX}{}", profile.id);

        create_from_profile(&client, &profile, None).expect("create session");

        // pane-died is a window-level hook — use show-hooks -w
        let hooks = client
            .cmd(&["show-hooks", "-w", "-t", &session_name])
            .unwrap();
        assert!(
            hooks.contains("pane-died"),
            "pane-died hook not found in: {hooks}"
        );
        assert!(
            hooks.contains("muster _pane-died"),
            "hook should invoke muster _pane-died: {hooks}"
        );

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_create_session_with_pane_layout() {
        use crate::config::profile::PaneProfile;

        let client = TmuxClient::new().expect("tmux must be installed");
        let profile = Profile {
            id: format!("test_{}", uuid::Uuid::new_v4()),
            name: "Pane Layout Test".to_string(),
            color: "#00ff00".to_string(),
            tabs: vec![TabProfile {
                name: "Dev".to_string(),
                cwd: "/tmp".to_string(),
                command: None,
                layout: None, // no layout string — just split
                panes: vec![
                    PaneProfile {
                        cwd: Some("/tmp".to_string()),
                        command: None,
                    },
                    PaneProfile {
                        cwd: Some("/var".to_string()),
                        command: None,
                    },
                ],
            }],
        };
        let session_name = format!("{SESSION_PREFIX}{}", profile.id);

        let info = create_from_profile(&client, &profile, None).expect("create session");
        assert_eq!(info.display_name, "Pane Layout Test");
        assert_eq!(info.window_count, 1);

        // Verify the window has 2 panes
        let panes = client.list_window_panes(&session_name, 0).unwrap();
        assert_eq!(
            panes.len(),
            2,
            "expected 2 panes, got {}: {:?}",
            panes.len(),
            panes
        );

        // Cleanup
        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_pane_died_hook_fires_on_process_exit() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("{SESSION_PREFIX}hookfire_{}", uuid::Uuid::new_v4());
        let anchor = format!("{SESSION_PREFIX}anchor_{}", uuid::Uuid::new_v4());
        let marker = format!("/tmp/muster_test_{}", uuid::Uuid::new_v4());

        // Anchor session keeps the tmux server alive while other parallel tests
        // create and destroy sessions (server exits when the last session dies).
        client
            .new_session(&anchor, "anchor", "/tmp", None)
            .expect("create anchor session");

        // Use /bin/sh directly to avoid default-shell startup overhead (fish
        // config loading can push total time well past 2s).
        client
            .new_session(&session_name, "test", "/tmp", Some("/bin/sh -c 'sleep 1'"))
            .expect("create session");

        // Set remain-on-exit and hook immediately (sleep is still running)
        client
            .cmd(&["set-option", "-t", &session_name, "remain-on-exit", "on"])
            .unwrap();
        let hook_cmd = format!("run-shell -b \"touch {marker}\"");
        client
            .cmd(&["set-hook", "-t", &session_name, "pane-died", &hook_cmd])
            .unwrap();

        // Poll for the marker file instead of a fixed sleep — more robust
        // across different systems and load levels.
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(8);
        while std::time::Instant::now() < deadline {
            if std::path::Path::new(&marker).exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        assert!(
            std::path::Path::new(&marker).exists(),
            "pane-died hook did not fire (marker file not created)"
        );

        // Cleanup
        std::fs::remove_file(&marker).ok();
        client.kill_session(&session_name).ok();
        client.kill_session(&anchor).ok();
    }
}
