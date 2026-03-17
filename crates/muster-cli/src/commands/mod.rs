pub(crate) mod attach;
pub(crate) mod color;
pub(crate) mod hooks;
pub(crate) mod inspect;
pub(crate) mod kill;
pub(crate) mod launch;
pub(crate) mod list;
pub(crate) mod new;
pub(crate) mod notifications;
pub(crate) mod peek;
pub(crate) mod pin;
pub(crate) mod profile;
pub(crate) mod settings;
pub(crate) mod status;

use std::path::PathBuf;

use muster::{Muster, SessionInfo, Settings};

use crate::error::bail;

/// Shared context passed to every command handler.
pub(crate) struct CommandContext {
    pub muster: Muster,
    pub settings: Settings,
    pub config_dir: PathBuf,
    pub json: bool,
}

/// Filter sessions by profile name, ID, or session name.
/// Returns an error if a filter is provided but matches nothing.
pub(crate) fn filter_sessions(
    sessions: &mut Vec<SessionInfo>,
    filter: Option<&str>,
) -> crate::error::Result {
    if let Some(filter) = filter {
        sessions.retain(|s| {
            s.display_name == *filter
                || s.profile_id.as_deref() == Some(filter)
                || s.session_name == *filter
        });
        if sessions.is_empty() {
            bail!("No session found for: {filter}");
        }
    }
    Ok(())
}
