use crate::error::{Error, Result};
use crate::tmux::client::TmuxClient;

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

/// Build the list of tmux set-option commands for theming a session.
pub fn build_theme_commands(
    session: &str,
    color: &str,
    display_name: &str,
) -> Result<Vec<Vec<String>>> {
    let (r, g, b) = hex_to_rgb(color)?;
    let (dr, dg, db) = compute_dimmed(r, g, b);
    let darker = rgb_to_hex(dr, dg, db);

    let commands = vec![
        vec![
            "set-option".into(),
            "-t".into(),
            session.into(),
            "status-style".into(),
            format!("bg={color},fg=#000000"),
        ],
        vec![
            "set-option".into(),
            "-t".into(),
            session.into(),
            "status-left".into(),
            format!("#[bg={darker},fg=#ffffff,bold] {display_name} #[default]"),
        ],
        vec![
            "set-option".into(),
            "-t".into(),
            session.into(),
            "window-status-format".into(),
            "#[fg=#000000]  #I: #W  ".into(),
        ],
        vec![
            "set-option".into(),
            "-t".into(),
            session.into(),
            "window-status-current-format".into(),
            format!("#[fg={color},bg=#000000,bold] #I: #W #[default]"),
        ],
        vec![
            "set-option".into(),
            "-t".into(),
            session.into(),
            "window-status-separator".into(),
            String::new(),
        ],
        vec![
            "set-option".into(),
            "-t".into(),
            session.into(),
            "status-position".into(),
            "top".into(),
        ],
    ];
    Ok(commands)
}

/// Apply the color theme to a running tmux session.
pub fn apply_theme(
    client: &TmuxClient,
    session: &str,
    color: &str,
    display_name: &str,
) -> Result<()> {
    let commands = build_theme_commands(session, color, display_name)?;
    for cmd in &commands {
        let args: Vec<&str> = cmd.iter().map(String::as_str).collect();
        client.cmd(&args)?;
    }
    Ok(())
}

/// Change a session's color: update the `@muster_color` option and re-apply theme.
pub fn set_color(
    client: &TmuxClient,
    session: &str,
    color: &str,
    display_name: &str,
) -> Result<()> {
    // Validate the color first
    hex_to_rgb(color)?;
    client.set_option(session, "@muster_color", color)?;
    apply_theme(client, session, color, display_name)
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
    fn test_build_theme_commands() {
        let commands = build_theme_commands("muster_test", "#f97316", "PKM Project").unwrap();
        assert_eq!(commands.len(), 6);

        // status-style
        assert_eq!(commands[0][3], "status-style");
        assert!(commands[0][4].contains("bg=#f97316"));

        // status-left includes display name and darker color
        assert_eq!(commands[1][3], "status-left");
        assert!(commands[1][4].contains("PKM Project"));
        assert!(commands[1][4].contains("#532607")); // dimmed

        // window-status-current-format includes the color
        assert_eq!(commands[3][3], "window-status-current-format");
        assert!(commands[3][4].contains("#f97316"));

        // status-position
        assert_eq!(commands[5][3], "status-position");
        assert_eq!(commands[5][4], "top");
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
