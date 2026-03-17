use super::CommandContext;
use crate::terminal::resolve_terminal;

pub(crate) fn execute(
    ctx: &CommandContext,
    terminal: Option<&str>,
    shell: Option<&str>,
    tmux_path: Option<&str>,
) -> crate::error::Result {
    let has_updates = terminal.is_some() || shell.is_some() || tmux_path.is_some();

    if has_updates {
        let mut settings = ctx.muster.settings().unwrap_or_default();

        if let Some(t) = terminal {
            settings.terminal = Some(t.to_string());
        }
        if let Some(s) = shell {
            settings.shell = Some(s.to_string());
        }
        if let Some(p) = tmux_path {
            settings.tmux_path = Some(p.to_string());
        }

        ctx.muster.save_settings(&settings)?;

        if !ctx.json {
            println!("Settings updated.");
        }
    }

    // Always show current settings after any update (or if no flags given)
    let settings = ctx.muster.settings().unwrap_or_default();

    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&settings)?);
    } else {
        let terminal_display = resolve_terminal(&settings);
        let is_default = settings.terminal.is_none();
        println!(
            "terminal: {}{}",
            terminal_display,
            if is_default { " (default)" } else { "" }
        );
        println!(
            "shell:    {}",
            settings.shell.as_deref().unwrap_or("(system default)")
        );
        println!(
            "tmux:     {}",
            settings
                .tmux_path
                .as_deref()
                .unwrap_or("(discovered from PATH)")
        );
    }

    Ok(())
}
