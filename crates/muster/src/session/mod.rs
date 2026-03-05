pub mod theme;

use crate::config::profile::{Profile, TabProfile};
use crate::error::Result;
use crate::tmux::client::{quote_tmux, quote_tmux_cmd, TmuxClient, SESSION_PREFIX};
use crate::tmux::types::{SessionInfo, TmuxWindow};

/// Expand a leading `~` to the user's home directory.
///
/// tmux does not properly expand `~/path` in `-c` arguments — it resolves
/// to just `$HOME` instead of `$HOME/path`. We expand it ourselves.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{rest}", home.display());
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

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

/// Build tmux command strings for pane setup within a window.
///
/// If the tab has panes defined, emits split-window, select-layout, and
/// send-keys commands. Otherwise, emits a single send-keys for the tab command.
fn build_window_pane_commands(
    session_name: &str,
    window_index: u32,
    tab: &TabProfile,
    shell: Option<&str>,
) -> Vec<String> {
    let mut commands = Vec::new();

    if tab.panes.is_empty() {
        // Single-pane behavior: send tab-level command
        if let Some(ref cmd) = tab.command {
            let target = format!("{session_name}:{window_index}");
            commands.push(format!(
                "send-keys -t {} {} Enter",
                target,
                quote_tmux(cmd),
            ));
        }
        return commands;
    }

    // Multi-pane: create splits for panes after the first
    for pane in tab.panes.iter().skip(1) {
        let raw_cwd = pane.cwd.as_deref().unwrap_or(&tab.cwd);
        let cwd = expand_tilde(raw_cwd);
        let target = format!("{session_name}:{window_index}");
        let mut cmd = format!("split-window -t {} -c {}", target, quote_tmux(&cwd));
        if let Some(sh) = shell {
            cmd.push(' ');
            cmd.push_str(&quote_tmux(sh));
        }
        commands.push(cmd);
    }

    // Apply layout (must happen after all splits are created)
    if let Some(ref layout) = tab.layout {
        let target = format!("{session_name}:{window_index}");
        commands.push(format!(
            "select-layout -t {} {}",
            target,
            quote_tmux(layout),
        ));
    }

    // Send commands to each pane
    for (pane_idx, pane) in tab.panes.iter().enumerate() {
        let target = format!("{session_name}:{window_index}.{pane_idx}");

        // If pane 0 has a different cwd than the tab, cd into it
        if pane_idx == 0 {
            if let Some(ref pane_cwd) = pane.cwd {
                if pane_cwd != &tab.cwd {
                    let resolved = expand_tilde(pane_cwd);
                    commands.push(format!(
                        "send-keys -t {} {} Enter",
                        target,
                        quote_tmux(&format!("cd {resolved}")),
                    ));
                }
            }
        }

        if let Some(ref cmd) = pane.command {
            commands.push(format!(
                "send-keys -t {} {} Enter",
                target,
                quote_tmux(cmd),
            ));
        }
    }

    commands
}

/// Build the complete list of tmux commands for a launch batch.
///
/// This produces all commands that should run after the initial `new-session`:
/// - default-command (if shell specified)
/// - Per-window pane setup (splits, layouts, send-keys)
/// - new-window for additional tabs
/// - Session metadata (@muster_name, @muster_color, @muster_profile)
/// - Per-window pinned markers and tab names
/// - Notification hooks (remain-on-exit, pane-died, alert-bell)
/// - Theme commands (session styling, per-window styling, theme hooks)
fn build_launch_commands(
    session_name: &str,
    profile: &Profile,
    shell: Option<&str>,
) -> Result<Vec<String>> {
    let mut commands = Vec::new();

    let first_tab = profile.tabs.first().cloned().unwrap_or_else(|| TabProfile {
        name: "Shell".to_string(),
        cwd: "/tmp".to_string(),
        command: None,
        layout: None,
        panes: vec![],
    });

    // Set default-command so panes the user creates manually also use the right shell
    if let Some(sh) = shell {
        commands.push(format!(
            "set-option -t {} default-command {}",
            session_name,
            quote_tmux(sh),
        ));
    }

    // Set up first window (panes or single-pane)
    commands.extend(build_window_pane_commands(
        session_name,
        0,
        &first_tab,
        shell,
    ));

    // Create additional windows and set up their panes
    for (i, tab) in profile.tabs.iter().enumerate().skip(1) {
        let index = u32::try_from(i).unwrap_or(0);
        let tab_cwd = expand_tilde(&tab.cwd);
        let mut new_win_cmd = format!(
            "new-window -t {} -n {} -c {}",
            session_name,
            quote_tmux(&tab.name),
            quote_tmux(&tab_cwd),
        );
        if let Some(sh) = shell {
            new_win_cmd.push(' ');
            new_win_cmd.push_str(&quote_tmux(sh));
        }
        commands.push(new_win_cmd);
        commands.extend(build_window_pane_commands(session_name, index, tab, shell));
    }

    // Set metadata
    commands.push(format!(
        "set-option -t {} @muster_name {}",
        session_name,
        quote_tmux(&profile.name),
    ));
    commands.push(format!(
        "set-option -t {} @muster_color {}",
        session_name,
        quote_tmux(&profile.color),
    ));
    commands.push(format!(
        "set-option -t {} @muster_profile {}",
        session_name,
        quote_tmux(&profile.id),
    ));

    // Mark all profile-created windows as pinned
    let window_count = profile.tabs.len().max(1);
    for (i, tab) in profile.tabs.iter().enumerate() {
        let target = format!("{session_name}:{i}");
        commands.push(format!("set-window-option -t {target} @muster_pinned 1"));
        commands.push(format!(
            "set-window-option -t {target} @muster_tab_name {}",
            quote_tmux(&tab.name),
        ));
    }
    // Handle the case where profile has no tabs (default Shell tab)
    if profile.tabs.is_empty() {
        let target = format!("{session_name}:0");
        commands.push(format!("set-window-option -t {target} @muster_pinned 1"));
        commands.push(format!(
            "set-window-option -t {target} @muster_tab_name {}",
            quote_tmux("Shell"),
        ));
    }

    // Notification hooks: remain-on-exit, pane-died, alert-bell
    commands.extend(build_hook_commands(session_name, window_count));

    // Theme commands
    commands.extend(theme::build_launch_theme_commands(
        session_name,
        &profile.color,
        &profile.name,
        window_count,
    )?);

    // Select window 0 so the session starts on the first tab
    commands.push(format!("select-window -t {session_name}:0"));

    Ok(commands)
}

/// Create a tmux session from a profile.
///
/// Steps:
/// 1. Create detached session with first tab (standalone — starts the server)
/// 2. Batch everything else via `source-file` (one spawn for ~50 commands)
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

    // Strip Claude Code env vars from the tmux server's global environment
    // *before* creating the session, so the first window's pane doesn't inherit
    // them. If no tmux server is running yet, these fail silently (the
    // env_remove on the Command spawn handles that case).
    for var in ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"] {
        let _ = client.cmd(&["set-environment", "-g", "-u", var]);
    }

    // Create the session with the first window (must be standalone — starts the server)
    let first_cwd = expand_tilde(&first_tab.cwd);
    client.new_session(&session_name, &first_tab.name, &first_cwd, shell)?;

    // Also unset at session level (in case server was just started by new_session
    // and inherited the vars from the process environment).
    for var in ["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"] {
        let _ = client.cmd(&["set-environment", "-g", "-u", var]);
        let _ = client.cmd(&["set-environment", "-t", &session_name, "-u", var]);
    }

    // Build and execute all remaining commands as a batch
    let commands = build_launch_commands(&session_name, profile, shell)?;
    client.source_file(&commands)?;

    let window_count = u32::try_from(profile.tabs.len().max(1)).unwrap_or(1);

    // Return session info
    Ok(SessionInfo {
        session_name,
        display_name: profile.name.clone(),
        color: profile.color.clone(),
        profile_id: Some(profile.id.clone()),
        window_count,
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

/// Build tmux command strings for session notification hooks.
///
/// Produces per-window `remain-on-exit on` and `pane-died` hooks,
/// plus a session-level `alert-bell` hook.
///
/// `pane-died` is a window-level hook and `remain-on-exit` is a pane option,
/// so both must be set on each window explicitly to cover all tabs.
fn build_hook_commands(session_name: &str, window_count: usize) -> Vec<String> {
    let pane_died_hook = concat!(
        "run-shell -b \"muster _pane-died",
        " #{session_name} '#{window_name}' #{pane_id} #{pane_dead_status}\""
    );

    let bell_hook = concat!(
        "run-shell -b \"muster _bell",
        " #{session_name} '#{window_name}'\""
    );

    let pane_died_quoted = quote_tmux_cmd(pane_died_hook);

    let mut commands = Vec::new();

    // Per-window: remain-on-exit and pane-died hook
    for i in 0..window_count {
        let target = format!("{session_name}:{i}");
        commands.push(format!("set-option -w -t {target} remain-on-exit on"));
        commands.push(format!("set-hook -w -t {target} pane-died {pane_died_quoted}"));
    }

    // Session-level alert-bell hook
    commands.push(format!(
        "set-hook -t {} alert-bell {}",
        session_name,
        quote_tmux_cmd(bell_hook),
    ));

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ensure_anchor() {
        let Ok(client) = TmuxClient::new() else { return };
        let _ = client.new_session("muster_test_anchor", "anchor", "/tmp", None);
        let _ = client.cmd(&["set-option", "-s", "exit-empty", "off"]);
    }

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
        ensure_anchor();
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
        ensure_anchor();
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
        ensure_anchor();
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
        ensure_anchor();
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
        ensure_anchor();
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
        ensure_anchor();
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
        ensure_anchor();
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

        ensure_anchor();
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
        ensure_anchor();
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("{SESSION_PREFIX}hookfire_{}", uuid::Uuid::new_v4());
        let marker = format!("/tmp/muster_test_{}", uuid::Uuid::new_v4());

        // Use /bin/sh directly to avoid default-shell startup overhead (fish
        // config loading can push total time well past 2s).
        // sleep 3 gives ample time for the set-option/set-hook commands below
        // to complete before the process exits, even under heavy parallel test load.
        client
            .new_session(&session_name, "test", "/tmp", Some("/bin/sh -c 'sleep 3'"))
            .expect("create session");

        // Set remain-on-exit and hook immediately (sleep is still running).
        // These MUST complete before sleep exits, otherwise pane-died won't fire.
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
            std::time::Instant::now() + std::time::Duration::from_secs(15);
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
    }
}
