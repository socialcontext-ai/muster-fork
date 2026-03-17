use super::CommandContext;
use crate::terminal::{resolve_terminal, send_notification};

pub(crate) fn execute_sync_rename(
    ctx: &CommandContext,
    session: &str,
    window: u32,
    name: &str,
) -> crate::error::Result {
    ctx.muster.sync_rename(session, window, name)?;
    Ok(())
}

pub(crate) fn execute_pane_died(
    ctx: &CommandContext,
    session_name: &str,
    window_name: &str,
    pane_id: &str,
    exit_code: i32,
) -> crate::error::Result {
    let display_name = ctx
        .muster
        .client()
        .get_option(session_name, "@muster_name")?
        .unwrap_or_else(|| session_name.to_string());

    // Capture last output from the dying pane before kill
    let snapshot = ctx
        .muster
        .client()
        .capture_pane(pane_id, 50)
        .unwrap_or_default();

    // Save snapshot to logs directory
    let log_dir = ctx.config_dir.join("logs").join(session_name);
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join(format!("{window_name}.last"));
    let _ = std::fs::write(&log_file, &snapshot);

    // Include last few lines in notification body
    let last_lines: String = snapshot
        .lines()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let body = if last_lines.is_empty() {
        format!("Exit code: {exit_code}")
    } else {
        format!("Exit code: {exit_code}\n{last_lines}")
    };

    let terminal = resolve_terminal(&ctx.settings);
    let summary = format!("Exited: {display_name} \u{25b8} {window_name}");
    send_notification(&summary, &body, session_name, window_name, &terminal);

    // Clean up the dead pane
    let _ = ctx.muster.client().cmd(&["kill-pane", "-t", pane_id]);

    Ok(())
}

pub(crate) fn execute_bell(
    ctx: &CommandContext,
    session_name: &str,
    window_name: &str,
) -> crate::error::Result {
    let display_name = ctx
        .muster
        .client()
        .get_option(session_name, "@muster_name")?
        .unwrap_or_else(|| session_name.to_string());

    let terminal = resolve_terminal(&ctx.settings);
    let summary = format!("Bell: {display_name} \u{25b8} {window_name}");

    send_notification(&summary, "", session_name, window_name, &terminal);

    Ok(())
}
