use std::io::IsTerminal;

use super::CommandContext;
use crate::format::color_dot;

pub(crate) fn execute(ctx: &CommandContext) -> crate::error::Result {
    let sessions = ctx.muster.list_sessions()?;
    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
    } else if sessions.is_empty() {
        println!("No active sessions.");
    } else {
        for s in &sessions {
            println!(
                "{} {} — {} [{} windows]",
                color_dot(&s.color),
                s.session_name,
                s.display_name,
                s.window_count,
            );
            if let Ok(windows) = ctx.muster.client().list_windows(&s.session_name) {
                for w in &windows {
                    let marker = if w.active { "→" } else { " " };
                    let stale = ctx
                        .muster
                        .client()
                        .get_window_option(&s.session_name, w.index, "@muster_layout_stale")
                        .ok()
                        .flatten()
                        .is_some();
                    let stale_tag = if stale {
                        if std::io::stdout().is_terminal() {
                            " \x1b[33;1m(layout unsaved)\x1b[0m"
                        } else {
                            " (layout unsaved)"
                        }
                    } else {
                        ""
                    };
                    println!("  {marker} {}: {} ({}){stale_tag}", w.index, w.name, w.cwd);
                }
            }
        }
    }

    Ok(())
}
