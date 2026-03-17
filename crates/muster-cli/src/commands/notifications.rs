use super::CommandContext;
use crate::terminal::{send_notification, setup_notifications, uninstall_notifications};

pub(crate) fn execute_setup() -> Result<(), Box<dyn std::error::Error>> {
    setup_notifications()
}

pub(crate) fn execute_remove() -> Result<(), Box<dyn std::error::Error>> {
    uninstall_notifications()
}

pub(crate) fn execute_test(ctx: &CommandContext) -> Result<(), Box<dyn std::error::Error>> {
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
