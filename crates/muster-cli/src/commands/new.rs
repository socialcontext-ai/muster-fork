use super::CommandContext;
use crate::tabs::build_tabs;
use crate::terminal::exec_tmux_attach;

pub(crate) fn execute(
    ctx: &CommandContext,
    name: &str,
    tab: &[String],
    color: &str,
    detach: bool,
) -> crate::error::Result {
    let tabs = build_tabs(tab)?;
    let color = muster::session::theme::resolve_color(color)?;

    let profile = muster::Profile {
        id: muster::config::profile::slugify(name),
        name: name.to_string(),
        color,
        tabs,
    };

    ctx.muster.save_profile(profile.clone())?;
    let info = ctx.muster.launch(&profile.id)?;

    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else if detach {
        println!("Created: {} ({})", info.display_name, info.session_name);
    } else {
        exec_tmux_attach(&info.session_name, &ctx.settings);
    }

    Ok(())
}
