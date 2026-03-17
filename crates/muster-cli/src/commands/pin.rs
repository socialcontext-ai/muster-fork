use super::CommandContext;

pub(crate) fn execute_pin(ctx: &CommandContext) -> Result<(), Box<dyn std::error::Error>> {
    let result = ctx.muster.pin_window()?;
    if !ctx.json {
        match result {
            muster::PinResult::Pinned => println!("Window pinned to profile."),
            muster::PinResult::LayoutUpdated => println!("Layout saved to profile."),
            muster::PinResult::AlreadyCurrent => println!("Layout already up to date."),
        }
    }
    Ok(())
}

pub(crate) fn execute_unpin(ctx: &CommandContext) -> Result<(), Box<dyn std::error::Error>> {
    ctx.muster.unpin_window()?;
    if !ctx.json {
        println!("Window unpinned from profile.");
    }
    Ok(())
}
