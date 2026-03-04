use crate::error::{Error, Result};
use crate::tmux::client::TmuxClient;

/// Resolve a named color to its hex value. Returns None if not a known name.
fn named_color_to_hex(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "black" => Some("#000000"),
        "red" => Some("#cc0000"),
        "green" => Some("#4e9a06"),
        "yellow" => Some("#c4a000"),
        "blue" => Some("#3465a4"),
        "magenta" => Some("#75507b"),
        "cyan" => Some("#06989a"),
        "white" => Some("#d3d7cf"),
        "orange" => Some("#f97316"),
        "pink" => Some("#ec4899"),
        "purple" | "violet" => Some("#8b5cf6"),
        "teal" => Some("#14b8a6"),
        "lime" => Some("#84cc16"),
        "amber" => Some("#f59e0b"),
        "rose" => Some("#f43f5e"),
        "indigo" => Some("#6366f1"),
        "sky" => Some("#0ea5e9"),
        "emerald" => Some("#10b981"),
        "fuchsia" => Some("#d946ef"),
        "slate" => Some("#64748b"),
        "zinc" => Some("#71717a"),
        "stone" => Some("#78716c"),
        "gray" | "grey" => Some("#808080"),
        _ => None,
    }
}

/// Resolve a color string to hex. Accepts `#RRGGBB`, `RRGGBB`, or a named color.
pub fn resolve_color(color: &str) -> Result<String> {
    if let Some(hex) = named_color_to_hex(color) {
        return Ok(hex.to_string());
    }
    // Validate as hex
    hex_to_rgb(color)?;
    // Normalize to #RRGGBB
    if color.starts_with('#') {
        Ok(color.to_string())
    } else {
        Ok(format!("#{color}"))
    }
}

/// Parse a hex color string (#RRGGBB) into (r, g, b).
pub fn hex_to_rgb(hex: &str) -> Result<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 {
        return Err(Error::InvalidColor(format!("#{hex}")));
    }
    let r =
        u8::from_str_radix(&hex[0..2], 16).map_err(|_| Error::InvalidColor(format!("#{hex}")))?;
    let g =
        u8::from_str_radix(&hex[2..4], 16).map_err(|_| Error::InvalidColor(format!("#{hex}")))?;
    let b =
        u8::from_str_radix(&hex[4..6], 16).map_err(|_| Error::InvalidColor(format!("#{hex}")))?;
    Ok((r, g, b))
}

/// Convert RGB back to hex string.
pub fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Compute a dimmed version of a color (divide each channel by 3).
pub fn compute_dimmed(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    (r / 3, g / 3, b / 3)
}

/// Compute relative luminance and return black or white for best contrast.
pub fn contrast_fg(r: u8, g: u8, b: u8) -> &'static str {
    // Relative luminance formula (simplified sRGB)
    let luminance = 0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b);
    if luminance > 128.0 {
        "#000000"
    } else {
        "#ffffff"
    }
}

/// Compute the theme values from a color and display name.
pub struct ThemeValues {
    pub color: String,
    pub fg: String,
    pub darker: String,
    pub display_name: String,
}

impl ThemeValues {
    pub fn new(color: &str, display_name: &str) -> Result<Self> {
        let (r, g, b) = hex_to_rgb(color)?;
        let (dr, dg, db) = compute_dimmed(r, g, b);
        Ok(Self {
            color: color.to_string(),
            fg: contrast_fg(r, g, b).to_string(),
            darker: rgb_to_hex(dr, dg, db),
            display_name: display_name.to_string(),
        })
    }

    /// Session-level options (status bar, position, mouse).
    pub fn session_commands(&self, session: &str) -> Vec<Vec<String>> {
        vec![
            vec![
                "set-option".into(),
                "-t".into(),
                session.into(),
                "status-style".into(),
                format!("bg={},fg={}", self.color, self.fg),
            ],
            vec![
                "set-option".into(),
                "-t".into(),
                session.into(),
                "status-left".into(),
                format!(
                    "#[bg={},fg=#ffffff,bold] {} #[default]",
                    self.darker, self.display_name
                ),
            ],
            vec![
                "set-option".into(),
                "-t".into(),
                session.into(),
                "status-position".into(),
                "top".into(),
            ],
            vec![
                "set-option".into(),
                "-t".into(),
                session.into(),
                "mouse".into(),
                "on".into(),
            ],
        ]
    }

    /// Stale layout indicator: yellow dot, shown conditionally via tmux format.
    const STALE_INDICATOR: &'static str =
        "#{?#{@muster_layout_stale},#[fg=#c4a000]\u{25cf} ,}";

    /// Window-level option key/value pairs for window-status styling.
    pub fn window_options(&self) -> Vec<(String, String)> {
        vec![
            (
                "window-status-format".into(),
                format!(
                    "#[bg={},fg={}]  #I: #W {}",
                    self.color, self.fg, Self::STALE_INDICATOR
                ),
            ),
            (
                "window-status-current-format".into(),
                format!(
                    "#[fg={},bg=#000000,bold]  #I: #W {}#[default]",
                    self.color, Self::STALE_INDICATOR
                ),
            ),
            ("window-status-separator".into(), String::new()),
        ]
    }

    /// Build the hook command string that applies window options to new windows.
    pub fn hook_command(&self) -> String {
        self.window_options()
            .iter()
            .map(|(k, v)| {
                if v.is_empty() {
                    format!("set-window-option {k} ''")
                } else {
                    format!("set-window-option {k} '{v}'")
                }
            })
            .collect::<Vec<_>>()
            .join(" ; ")
    }

    /// Window-level options for unpinned (neutral) windows.
    /// Includes a red beacon (●) to signal the window is ephemeral.
    pub fn neutral_window_options() -> Vec<(String, String)> {
        vec![
            (
                "window-status-format".into(),
                "#[bg=#333333,fg=#999999]  #I: #W #[fg=#cc0000]● ".into(),
            ),
            (
                "window-status-current-format".into(),
                "#[fg=#999999,bg=#000000,bold]  #I: #W #[fg=#cc0000]● #[default]".into(),
            ),
            ("window-status-separator".into(), String::new()),
        ]
    }

    /// Build hook command that applies neutral styling to new windows.
    pub fn neutral_hook_command() -> String {
        Self::neutral_window_options()
            .iter()
            .map(|(k, v)| {
                if v.is_empty() {
                    format!("set-window-option {k} ''")
                } else {
                    format!("set-window-option {k} '{v}'")
                }
            })
            .collect::<Vec<_>>()
            .join(" ; ")
    }
}

/// Build the list of tmux commands for theming a session (for testing).
pub fn build_theme_commands(
    session: &str,
    color: &str,
    display_name: &str,
) -> Result<Vec<Vec<String>>> {
    let tv = ThemeValues::new(color, display_name)?;
    let mut commands = tv.session_commands(session);
    // Include window options as set-option for backward compat in tests
    for (k, v) in tv.window_options() {
        commands.push(vec!["set-option".into(), "-t".into(), session.into(), k, v]);
    }
    Ok(commands)
}

/// Apply colored (pinned) styling to a single window.
pub fn apply_pinned_window_style(
    client: &TmuxClient,
    session: &str,
    window_index: u32,
    color: &str,
    display_name: &str,
) -> Result<()> {
    let tv = ThemeValues::new(color, display_name)?;
    for (key, value) in &tv.window_options() {
        client.set_window_option(session, window_index, key, value)?;
    }
    client.set_window_option(session, window_index, "@muster_pinned", "1")?;
    Ok(())
}

/// Apply neutral (unpinned) styling to a single window.
pub fn apply_neutral_window_style(
    client: &TmuxClient,
    session: &str,
    window_index: u32,
) -> Result<()> {
    for (key, value) in &ThemeValues::neutral_window_options() {
        client.set_window_option(session, window_index, key, value)?;
    }
    // Best-effort removal — ignore errors if the options were never set
    let _ = client.unset_window_option(session, window_index, "@muster_pinned");
    let _ = client.unset_window_option(session, window_index, "@muster_tab_name");
    Ok(())
}

/// Apply the color theme to a running tmux session.
/// Accepts hex colors (`#f97316`) or named colors (`orange`).
///
/// Sets session-level options directly and applies window-level options
/// to each existing window (colored for pinned, neutral for unpinned),
/// plus installs a hook that gives new windows neutral styling.
pub fn apply_theme(
    client: &TmuxClient,
    session: &str,
    color: &str,
    display_name: &str,
) -> Result<()> {
    let color = resolve_color(color)?;
    let tv = ThemeValues::new(&color, display_name)?;

    // Apply session-level options
    for cmd in &tv.session_commands(session) {
        let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
        client.cmd(&args)?;
    }

    // Apply per-window styling: pinned windows get colored, unpinned get neutral
    let windows = client.list_windows(session)?;
    for win in &windows {
        let pinned = client
            .get_window_option(session, win.index, "@muster_pinned")?
            .is_some();
        if pinned {
            let target = format!("{session}:{}", win.index);
            for (key, value) in &tv.window_options() {
                client.cmd(&["set-window-option", "-t", &target, key, value])?;
            }
        } else {
            let target = format!("{session}:{}", win.index);
            for (key, value) in &ThemeValues::neutral_window_options() {
                client.cmd(&["set-window-option", "-t", &target, key, value])?;
            }
        }
    }

    // Hook gives new windows neutral styling (they're unpinned by default)
    client.cmd(&[
        "set-hook",
        "-t",
        session,
        "after-new-window",
        &ThemeValues::neutral_hook_command(),
    ])?;

    // Hook marks pinned windows as layout-stale when panes are split
    client.cmd(&[
        "set-hook",
        "-t",
        session,
        "after-split-window",
        "if-shell -F '#{@muster_pinned}' 'set-window-option @muster_layout_stale 1'",
    ])?;

    // Hook syncs window renames to the profile for pinned windows
    let muster_bin = std::env::current_exe()
        .unwrap_or_else(|_| "muster".into())
        .display()
        .to_string();
    let rename_hook = format!(
        "run-shell '{muster_bin} sync-rename \
         #{{session_name}} #{{window_index}} \"#{{window_name}}\"'"
    );
    client.cmd(&[
        "set-hook",
        "-t",
        session,
        "after-rename-window",
        &rename_hook,
    ])?;

    Ok(())
}

/// Change a session's color: update the `@muster_color` option and re-apply theme.
/// Accepts hex colors (`#f97316`) or named colors (`orange`).
pub fn set_color(
    client: &TmuxClient,
    session: &str,
    color: &str,
    display_name: &str,
) -> Result<()> {
    let color = resolve_color(color)?;
    client.set_option(session, "@muster_color", &color)?;
    apply_theme(client, session, &color, display_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_rgb() {
        assert_eq!(hex_to_rgb("#f97316").unwrap(), (249, 115, 22));
        assert_eq!(hex_to_rgb("#000000").unwrap(), (0, 0, 0));
        assert_eq!(hex_to_rgb("#ffffff").unwrap(), (255, 255, 255));
        assert_eq!(hex_to_rgb("ff0000").unwrap(), (255, 0, 0)); // without #
    }

    #[test]
    fn test_hex_to_rgb_invalid() {
        assert!(hex_to_rgb("#xyz").is_err());
        assert!(hex_to_rgb("#gggggg").is_err());
        assert!(hex_to_rgb("").is_err());
    }

    #[test]
    fn test_compute_dimmed_color() {
        assert_eq!(compute_dimmed(249, 115, 22), (83, 38, 7));
        assert_eq!(compute_dimmed(255, 255, 255), (85, 85, 85));
        assert_eq!(compute_dimmed(0, 0, 0), (0, 0, 0));
    }

    #[test]
    fn test_rgb_to_hex() {
        assert_eq!(rgb_to_hex(249, 115, 22), "#f97316");
        assert_eq!(rgb_to_hex(0, 0, 0), "#000000");
        assert_eq!(rgb_to_hex(255, 255, 255), "#ffffff");
    }

    #[test]
    fn test_resolve_color_hex() {
        assert_eq!(resolve_color("#f97316").unwrap(), "#f97316");
        assert_eq!(resolve_color("ff0000").unwrap(), "#ff0000");
    }

    #[test]
    fn test_resolve_color_named() {
        assert_eq!(resolve_color("orange").unwrap(), "#f97316");
        assert_eq!(resolve_color("Orange").unwrap(), "#f97316");
        assert_eq!(resolve_color("BLUE").unwrap(), "#3465a4");
        assert_eq!(resolve_color("gray").unwrap(), "#808080");
        assert_eq!(resolve_color("grey").unwrap(), "#808080");
    }

    #[test]
    fn test_resolve_color_invalid() {
        assert!(resolve_color("notacolor").is_err());
        assert!(resolve_color("").is_err());
    }

    #[test]
    fn test_build_theme_commands() {
        let commands = build_theme_commands("muster_test", "#f97316", "PKM Project").unwrap();
        assert_eq!(commands.len(), 7);

        // Session options come first
        assert_eq!(commands[0][3], "status-style");
        assert!(commands[0][4].contains("bg=#f97316"));

        assert_eq!(commands[1][3], "status-left");
        assert!(commands[1][4].contains("PKM Project"));
        assert!(commands[1][4].contains("#532607")); // dimmed

        assert_eq!(commands[2][3], "status-position");
        assert_eq!(commands[2][4], "top");

        assert_eq!(commands[3][3], "mouse");
        assert_eq!(commands[3][4], "on");

        // Window options follow
        assert_eq!(commands[4][3], "window-status-format");
        assert!(commands[4][4].contains("#f97316"));

        assert_eq!(commands[5][3], "window-status-current-format");
        assert!(commands[5][4].contains("#f97316"));
        assert!(commands[5][4].contains("bg=#000000"));

        assert_eq!(commands[6][3], "window-status-separator");
    }

    #[test]
    #[ignore]
    fn test_apply_theme() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "shell", "/tmp", None)
            .expect("create session");

        apply_theme(&client, &session_name, "#f97316", "Test").expect("apply theme");

        // Verify a tmux option was set
        let output = client
            .cmd(&["show-option", "-t", &session_name, "-v", "status-position"])
            .unwrap();
        assert_eq!(output.trim(), "top");

        client.kill_session(&session_name).ok();
    }

    #[test]
    #[ignore]
    fn test_change_color_live() {
        let client = TmuxClient::new().expect("tmux must be installed");
        let session_name = format!("muster_test_{}", uuid::Uuid::new_v4());
        client
            .new_session(&session_name, "shell", "/tmp", None)
            .expect("create session");

        set_color(&client, &session_name, "#00ff00", "Test").expect("set color");

        let color = client.get_option(&session_name, "@muster_color").unwrap();
        assert_eq!(color, Some("#00ff00".to_string()));

        client.kill_session(&session_name).ok();
    }
}
