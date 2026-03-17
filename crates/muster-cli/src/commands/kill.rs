use super::CommandContext;

pub(crate) fn execute(ctx: &CommandContext, session: &str) -> crate::error::Result {
    let session = ctx.muster.resolve_session(session)?;
    ctx.muster.destroy(&session)?;
    if !ctx.json {
        println!("Destroyed: {session}");
    }
    Ok(())
}
