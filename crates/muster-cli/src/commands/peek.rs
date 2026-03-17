use super::CommandContext;
use crate::error::bail;

pub(crate) fn execute(
    ctx: &CommandContext,
    session: &str,
    windows: &[String],
    lines: u32,
) -> crate::error::Result {
    let session_name = ctx.muster.resolve_session(session)?;
    let all_windows = ctx.muster.client().list_windows(&session_name)?;

    let targets: Vec<_> = if windows.is_empty() {
        all_windows.iter().collect()
    } else {
        all_windows
            .iter()
            .filter(|w| windows.iter().any(|name| w.name.eq_ignore_ascii_case(name)))
            .collect()
    };

    if targets.is_empty() {
        bail!("No matching windows found.");
    }

    if ctx.json {
        let entries: Vec<serde_json::Value> = targets
            .iter()
            .map(|win| {
                let target = format!("{}:{}", session_name, win.index);
                let output = ctx
                    .muster
                    .client()
                    .capture_pane(&target, lines)
                    .unwrap_or_default();
                serde_json::json!({
                    "window": win.name,
                    "index": win.index,
                    "output": output.trim_end(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        for (i, win) in targets.iter().enumerate() {
            if i > 0 {
                println!();
            }
            let header = format!("\u{2500}\u{2500} {} ", win.name);
            let pad = 40usize.saturating_sub(header.len());
            println!("{}{}", header, "\u{2500}".repeat(pad));
            let target = format!("{}:{}", session_name, win.index);
            match ctx.muster.client().capture_pane(&target, lines) {
                Ok(output) => {
                    let trimmed = output.trim_end();
                    if trimmed.is_empty() {
                        println!("(empty)");
                    } else {
                        println!("{trimmed}");
                    }
                }
                Err(e) => eprintln!("  (capture failed: {e})"),
            }
        }
    }

    Ok(())
}
