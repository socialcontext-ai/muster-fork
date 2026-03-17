use super::CommandContext;
use crate::terminal::{
    resolve_terminal, send_notification, setup_notifications, uninstall_notifications,
};

pub(crate) fn execute_setup() -> crate::error::Result {
    setup_notifications()
}

pub(crate) fn execute_remove() -> crate::error::Result {
    uninstall_notifications()
}

#[allow(clippy::unnecessary_wraps)]
pub(crate) fn execute_test(ctx: &CommandContext) -> crate::error::Result {
    let terminal = resolve_terminal(&ctx.settings);
    send_notification(
        "Muster Test",
        "Notifications are working.",
        "",
        "",
        &terminal,
    );
    println!("Test notification sent.");
    Ok(())
}
