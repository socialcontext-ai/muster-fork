use super::CommandContext;
use crate::terminal::{send_notification, setup_notifications, uninstall_notifications};

pub(crate) fn execute_setup() -> crate::error::Result {
    setup_notifications()
}

pub(crate) fn execute_remove() -> crate::error::Result {
    uninstall_notifications()
}

pub(crate) fn execute_test(ctx: &CommandContext) -> crate::error::Result {
    send_notification(
        "Muster Test",
        "Notifications are working.",
        "",
        "",
        &ctx.muster
            .settings()?
            .terminal
            .unwrap_or_else(|| "ghostty".to_string()),
    );
    println!("Test notification sent.");
    Ok(())
}
