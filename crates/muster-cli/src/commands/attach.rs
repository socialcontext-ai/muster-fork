use super::CommandContext;
use crate::terminal::exec_tmux_attach;

pub(crate) fn execute(
    ctx: &CommandContext,
    session: &str,
    tab: Option<u32>,
) -> crate::error::Result {
    let session_name = ctx.muster.resolve_session(session)?;

    if let Some(idx) = tab {
        ctx.muster.switch_window(&session_name, idx)?;
    }

    exec_tmux_attach(&session_name, &ctx.settings);
}
