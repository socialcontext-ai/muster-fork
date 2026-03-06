//! Tmux session theming: color-coded status bars, pinned/unpinned window
//! styling, and hook installation for live theme propagation.

use crate::error::{Error, Result};
use crate::tmux::client::TmuxClient;

/// The set of named colors available for session theming.
///
/// Each entry is `(canonical_name, aliases, hex)`. The canonical name is the
/// primary name shown in listings; aliases are accepted as synonyms.
pub const NAMED_COLORS: &[(&str, &[&str], &str)] = &[
    ("black", &[], "#000000"),
    ("red", &[], "#cc0000"),
    ("green", &[], "#4e9a06"),
    ("yellow", &[], "#c4a000"),
    ("blue", &[], "#3465a4"),
    ("magenta", &[], "#75507b"),
    ("cyan", &[], "#06989a"),
    ("white", &[], "#d3d7cf"),
    ("orange", &[], "#f97316"),
    ("pink", &[], "#ec4899"),
    ("purple", &["violet"], "#a855f7"),
    ("teal", &[], "#14b8a6"),
    ("lime", &[], "#84cc16"),
    ("amber", &[], "#f59e0b"),
    ("rose", &[], "#f43f5e"),
    ("indigo", &[], "#6366f1"),
    ("sky", &[], "#0ea5e9"),
    ("emerald", &[], "#10b981"),
    ("fuchsia", &[], "#d946ef"),
    ("coral", &[], "#ff7f50"),
    ("tomato", &[], "#ff6347"),
    ("crimson", &[], "#dc143c"),
    ("gold", &[], "#ffd700"),
    ("navy", &[], "#000080"),
    ("brown", &["chocolate"], "#8b4513"),
    ("slate", &[], "#64748b"),
    ("gray", &["grey"], "#808080"),
];

/// Tailwind CSS shade variants: light (300), base (500), dark (700).
///
/// Each entry is `(family_name, light_hex, base_hex, dark_hex)`.
/// Bare color names in `NAMED_COLORS` may differ from the base here
/// (e.g. bare `red` is the classic terminal value, not Tailwind red-500).
pub const TAILWIND_SHADES: &[(&str, &str, &str, &str)] = &[
    ("slate", "#cbd5e1", "#64748b", "#334155"),
    ("gray", "#d1d5db", "#6b7280", "#374151"),
    ("red", "#fca5a5", "#ef4444", "#b91c1c"),
    ("orange", "#fdba74", "#f97316", "#c2410c"),
    ("amber", "#fcd34d", "#f59e0b", "#b45309"),
    ("yellow", "#fde047", "#eab308", "#a16207"),
    ("lime", "#bef264", "#84cc16", "#4d7c0f"),
    ("green", "#86efac", "#22c55e", "#15803d"),
    ("emerald", "#6ee7b7", "#10b981", "#047857"),
    ("teal", "#5eead4", "#14b8a6", "#0f766e"),
    ("cyan", "#67e8f9", "#06b6d4", "#0e7490"),
    ("sky", "#7dd3fc", "#0ea5e9", "#0369a1"),
    ("blue", "#93c5fd", "#3b82f6", "#1d4ed8"),
    ("indigo", "#a5b4fc", "#6366f1", "#4338ca"),
    ("violet", "#c4b5fd", "#8b5cf6", "#6d28d9"),
    ("purple", "#d8b4fe", "#a855f7", "#7e22ce"),
    ("fuchsia", "#f0abfc", "#d946ef", "#a21caf"),
    ("pink", "#f9a8d4", "#ec4899", "#be185d"),
    ("rose", "#fda4af", "#f43f5e", "#be123c"),
];

/// Compute a lighter variant by mixing 50% toward white.
#[allow(clippy::cast_possible_truncation)] // midpoint of u8 values fits in u8
fn computed_light(hex: &str) -> Option<String> {
    let (r, g, b) = hex_to_rgb(hex).ok()?;
    let lr = u16::midpoint(u16::from(r), 255) as u8;
    let lg = u16::midpoint(u16::from(g), 255) as u8;
    let lb = u16::midpoint(u16::from(b), 255) as u8;
    Some(rgb_to_hex(lr, lg, lb))
}

/// Compute a darker variant by scaling channels to 45%.
#[allow(clippy::cast_possible_truncation)] // 45% of a u8 value fits in u8
fn computed_dark(hex: &str) -> Option<String> {
    let (r, g, b) = hex_to_rgb(hex).ok()?;
    let dr = (u16::from(r) * 45 / 100) as u8;
    let dg = (u16::from(g) * 45 / 100) as u8;
    let db = (u16::from(b) * 45 / 100) as u8;
    Some(rgb_to_hex(dr, dg, db))
}

/// Resolve a shaded color name (e.g. `red-light`, `blue-dark`) to hex.
///
/// Checks `TAILWIND_SHADES` first for curated values, then falls back to
/// computing light/dark from the base color in `NAMED_COLORS`.
fn shaded_color_to_hex(input: &str) -> Option<String> {
    let (name, suffix) = input.rsplit_once('-')?;
    if suffix != "light" && suffix != "dark" {
        return None;
    }
    let lower_name = name.to_lowercase();

    // Prefer curated Tailwind shades
    if let Some(scale) = TAILWIND_SHADES.iter().find(|(n, _, _, _)| *n == lower_name) {
        return match suffix {
            "light" => Some(scale.1.to_string()),
            "dark" => Some(scale.3.to_string()),
            _ => None,
        };
    }

    // Fall back to computing from the named color's base hex
    let base_hex = NAMED_COLORS.iter().find_map(|(canonical, aliases, hex)| {
        if *canonical == lower_name || aliases.contains(&lower_name.as_str()) {
            Some(*hex)
        } else {
            None
        }
    })?;
    match suffix {
        "light" => computed_light(base_hex),
        "dark" => computed_dark(base_hex),
        _ => None,
    }
}

/// Resolve a named color to its hex value. Returns None if not a known name.
///
/// Tries exact/alias match in `NAMED_COLORS` first, then falls back to
/// shade suffixes (`-light`, `-dark`).
fn named_color_to_hex(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    // Try exact/alias match first
    if let Some(hex) = NAMED_COLORS.iter().find_map(|(canonical, aliases, hex)| {
        if *canonical == lower || aliases.contains(&lower.as_str()) {
            Some(*hex)
        } else {
            None
        }
    }) {
        return Some(hex.to_string());
    }
    // Try shaded name (e.g. red-light, blue-dark)
    shaded_color_to_hex(&lower)
}

/// Resolve a color string to hex. Accepts `#RRGGBB`, `RRGGBB`, or a named color.
pub fn resolve_color(color: &str) -> Result<String> {
    if let Some(hex) = named_color_to_hex(color) {
        return Ok(hex);
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
fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Compute a dimmed version of a color (divide each channel by 3).
fn compute_dimmed(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    (r / 3, g / 3, b / 3)
}

/// Compute relative luminance and return black or white for best contrast.
fn contrast_fg(r: u8, g: u8, b: u8) -> &'static str {
    // Relative luminance formula (simplified sRGB)
    let luminance = 0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b);
    if luminance > 128.0 {
        "#000000"
    } else {
        "#ffffff"
    }
}

/// Compute the theme values from a color and display name.
pub(crate) struct ThemeValues {
    pub(crate) color: String,
    pub(crate) fg: String,
    pub(crate) darker: String,
    pub(crate) display_name: String,
}

impl ThemeValues {
    pub(crate) fn new(color: &str, display_name: &str) -> Result<Self> {
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
    pub(crate) fn session_commands(&self, session: &str) -> Vec<Vec<String>> {
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
    const STALE_INDICATOR: &'static str = "#{?#{@muster_layout_stale},#[fg=#c4a000]\u{25cf} ,}";

    /// Window-level option key/value pairs for window-status styling.
    pub(crate) fn window_options(&self) -> Vec<(String, String)> {
        vec![
            (
                "window-status-format".into(),
                format!(
                    "#[bg={},fg={}]  #I: #W {}",
                    self.color,
                    self.fg,
                    Self::STALE_INDICATOR
                ),
            ),
            (
                "window-status-current-format".into(),
                format!(
                    "#[fg={},bg=#000000,bold]  #I: #W {}#[default]",
                    self.color,
                    Self::STALE_INDICATOR
                ),
            ),
            ("window-status-separator".into(), String::new()),
        ]
    }

    /// Window-level options for unpinned (neutral) windows.
    /// Includes a red beacon (●) to signal the window is ephemeral.
    pub(crate) fn neutral_window_options() -> Vec<(String, String)> {
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
    pub(crate) fn neutral_hook_command() -> String {
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

/// Build tmux command strings for theming a newly-launched session.
///
/// During launch, all windows are pinned and the window count is known from
/// the profile. This avoids the `list_windows` + `get_window_option` queries
/// that `apply_theme` uses for live color changes.
pub(crate) fn build_launch_theme_commands(
    session: &str,
    color: &str,
    display_name: &str,
    window_count: usize,
) -> Result<Vec<String>> {
    use crate::tmux::client::{quote_tmux, quote_tmux_cmd};

    let color = resolve_color(color)?;
    let tv = ThemeValues::new(&color, display_name)?;

    let mut commands = Vec::new();

    // Session-level options (status-style, status-left, status-position, mouse)
    for cmd in &tv.session_commands(session) {
        let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
        // Format: "set-option -t session key value"
        // Last arg may contain spaces, so quote it
        let last = args.last().unwrap();
        let leading: Vec<&str> = args[..args.len() - 1].to_vec();
        commands.push(format!("{} {}", leading.join(" "), quote_tmux(last)));
    }

    // Per-window pinned styling (all windows are pinned at launch)
    for window_index in 0..window_count {
        let target = format!("{session}:{window_index}");
        for (key, value) in &tv.window_options() {
            commands.push(format!(
                "set-window-option -t {} {} {}",
                target,
                key,
                quote_tmux(value),
            ));
        }
    }

    // Hook: new windows get neutral styling (they're unpinned by default)
    // Use brace quoting for hook commands (they contain single quotes)
    commands.push(format!(
        "set-hook -t {} after-new-window {}",
        session,
        quote_tmux_cmd(&ThemeValues::neutral_hook_command()),
    ));

    // Hook: mark pinned windows as layout-stale when panes are split
    commands.push(format!(
        "set-hook -t {} after-split-window {}",
        session,
        quote_tmux_cmd(
            "if-shell -F '#{@muster_pinned}' 'set-window-option @muster_layout_stale 1'"
        ),
    ));

    // Hook: sync window renames to the profile for pinned windows
    let muster_bin = std::env::current_exe()
        .unwrap_or_else(|_| "muster".into())
        .display()
        .to_string();
    let rename_hook = format!(
        "run-shell '{muster_bin} sync-rename \
         #{{session_name}} #{{window_index}} \"#{{window_name}}\"'"
    );
    commands.push(format!(
        "set-hook -t {} after-rename-window {}",
        session,
        quote_tmux_cmd(&rename_hook),
    ));

    Ok(commands)
}

/// Build the list of tmux commands for theming a session (for testing).
#[cfg(test)]
fn build_theme_commands(
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
pub(crate) fn apply_pinned_window_style(
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
pub(crate) fn apply_neutral_window_style(
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
pub(crate) fn apply_theme(
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
pub(crate) fn set_color(
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
    fn test_resolve_color_shaded() {
        // Light shades (Tailwind 300)
        assert_eq!(resolve_color("red-light").unwrap(), "#fca5a5");
        assert_eq!(resolve_color("Blue-Light").unwrap(), "#93c5fd");
        // Dark shades (Tailwind 700)
        assert_eq!(resolve_color("red-dark").unwrap(), "#b91c1c");
        assert_eq!(resolve_color("Green-Dark").unwrap(), "#15803d");
        // Bare name still returns NAMED_COLORS value, not Tailwind base
        assert_eq!(resolve_color("red").unwrap(), "#cc0000");
        // Violet alias resolves to purple's hex
        assert_eq!(resolve_color("violet").unwrap(), "#a855f7");
        // Violet shades still work via TAILWIND_SHADES
        assert_eq!(resolve_color("violet-light").unwrap(), "#c4b5fd");
        assert_eq!(resolve_color("violet-dark").unwrap(), "#6d28d9");
        // New CSS named colors
        assert_eq!(resolve_color("coral").unwrap(), "#ff7f50");
        assert_eq!(resolve_color("navy").unwrap(), "#000080");
        assert_eq!(resolve_color("crimson").unwrap(), "#dc143c");
        assert_eq!(resolve_color("gold").unwrap(), "#ffd700");
        assert_eq!(resolve_color("brown").unwrap(), "#8b4513");
        assert_eq!(resolve_color("chocolate").unwrap(), "#8b4513");
        assert_eq!(resolve_color("tomato").unwrap(), "#ff6347");
        // Computed shades for colors not in TAILWIND_SHADES
        assert!(resolve_color("brown-light").is_ok());
        assert!(resolve_color("brown-dark").is_ok());
        assert!(resolve_color("navy-light").is_ok());
        assert!(resolve_color("coral-dark").is_ok());
    }

    #[test]
    fn test_shaded_color_invalid_suffix() {
        // Unknown suffix falls through to hex validation and fails
        assert!(resolve_color("red-medium").is_err());
        assert!(resolve_color("red-base").is_err());
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
    fn test_snapshot_theme_commands() {
        let commands = build_theme_commands("muster_test", "#f97316", "PKM Project").unwrap();
        insta::assert_json_snapshot!(commands);
    }

    fn ensure_anchor() {
        let Ok(client) = TmuxClient::new() else {
            return;
        };
        let _ = client.new_session("muster_test_anchor", "anchor", "/tmp", None);
        let _ = client.cmd(&["set-option", "-s", "exit-empty", "off"]);
    }

    #[test]
    #[ignore]
    fn test_apply_theme() {
        ensure_anchor();
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
        ensure_anchor();
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
