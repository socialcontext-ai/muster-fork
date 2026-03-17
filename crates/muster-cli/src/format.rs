use std::io::IsTerminal;

/// Render a colored dot using ANSI truecolor. Falls back to plain dot if not a TTY.
pub(crate) fn color_dot(hex: &str) -> String {
    if !std::io::stdout().is_terminal() {
        return "●".to_string();
    }
    if let Ok((r, g, b)) = muster::session::theme::hex_to_rgb(hex) {
        format!("\x1b[38;2;{r};{g};{b}m●\x1b[0m")
    } else {
        "●".to_string()
    }
}

/// Format bytes from KB to a human-readable string.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn format_memory(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.1} GB", kb as f64 / 1_048_576.0)
    } else if kb >= 1024 {
        format!("{:.1} MB", kb as f64 / 1024.0)
    } else {
        format!("{kb} KB")
    }
}
