use super::CommandContext;
use crate::error::bail;
use crate::terminal::exec_tmux_attach;

pub(crate) fn execute(
    ctx: &CommandContext,
    profile: &str,
    tab: Option<u32>,
    detach: bool,
) -> crate::error::Result {
    let profiles = ctx.muster.list_profiles()?;
    let found = profiles
        .iter()
        .find(|p| p.name == profile || p.id == profile);

    let Some(p) = found else {
        bail!("Profile not found: {profile}");
    };
    let profile_id = p.id.clone();

    let info = ctx.muster.launch(&profile_id)?;

    if let Some(idx) = tab {
        ctx.muster.switch_window(&info.session_name, idx)?;
    }

    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else if detach {
        println!("Launched: {} ({})", info.display_name, info.session_name);
    } else {
        exec_tmux_attach(&info.session_name, &ctx.settings);
    }

    Ok(())
}
