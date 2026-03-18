use std::io::Write as _;
use std::process;

use muster::{Muster, Profile, TabProfile};

use super::CommandContext;
use crate::editing::EditableProfile;
use crate::error::bail;
use crate::format::color_dot;
use crate::tabs::build_tabs;

/// Resolve a profile by name or ID.
fn resolve_profile(m: &Muster, input: &str) -> crate::error::Result<Profile> {
    let profiles = m.list_profiles()?;
    let found = profiles
        .into_iter()
        .find(|p| p.name == input || p.id == input);
    match found {
        Some(p) => Ok(p),
        None => bail!("Profile not found: {input}"),
    }
}

pub(crate) fn execute_list(ctx: &CommandContext) -> crate::error::Result {
    let profiles = ctx.muster.list_profiles()?;
    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&profiles)?);
    } else if profiles.is_empty() {
        println!("No profiles.");
    } else {
        for p in &profiles {
            println!(
                "  {} {} ({}) — {} tab(s)",
                color_dot(&p.color),
                p.name,
                p.id,
                p.tabs.len(),
            );
        }
    }
    Ok(())
}

pub(crate) fn execute_delete(ctx: &CommandContext, id: &str) -> crate::error::Result {
    let profiles = ctx.muster.list_profiles()?;
    let found = profiles.iter().find(|p| p.name == id || p.id == id);

    let Some(p) = found else {
        bail!("Profile not found: {id}");
    };

    let name = p.name.clone();
    ctx.muster.delete_profile(&p.id)?;
    if !ctx.json {
        println!("Deleted: {name}");
    }
    Ok(())
}

pub(crate) fn execute_save(
    ctx: &CommandContext,
    name: &str,
    tab: &[String],
    color: &str,
    from_session: Option<&str>,
) -> crate::error::Result {
    let color = muster::session::theme::resolve_color(color)?;

    let (tabs, live_session) = if let Some(session) = from_session {
        let session_name = ctx.muster.resolve_session(session)?;
        let tabs = ctx.muster.snapshot_session(&session_name)?;
        (tabs, Some(session_name))
    } else {
        (build_tabs(tab)?, None)
    };

    let profile = muster::Profile {
        id: muster::config::profile::slugify(name),
        name: name.to_string(),
        color,
        tabs,
        ..muster::Profile::default()
    };

    // Update existing profile if one already exists with this name/id,
    // otherwise create a new one. This handles the common workflow of
    // `muster new foo` followed by `muster profile save foo --from-session foo`.
    let saved = match ctx.muster.save_profile(profile.clone()) {
        Ok(p) => p,
        Err(muster::Error::DuplicateProfile(_)) => ctx.muster.update_profile(profile)?,
        Err(e) => return Err(e.into()),
    };

    // Pin the live session's windows now that the profile exists
    if let Some(session_name) = live_session {
        ctx.muster.pin_session_windows(&session_name)?;
    }

    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&saved)?);
    } else {
        println!("Saved: {} ({})", saved.name, saved.id);
    }
    Ok(())
}

pub(crate) fn execute_add_tab(
    ctx: &CommandContext,
    profile: &str,
    name: String,
    cwd: String,
    command: Option<String>,
) -> crate::error::Result {
    let profiles = ctx.muster.list_profiles()?;
    let found = profiles
        .iter()
        .find(|p| p.name == profile || p.id == profile);

    let Some(p) = found else {
        bail!("Profile not found: {profile}");
    };

    let cwd = if cwd == "." {
        std::env::current_dir()?.to_string_lossy().to_string()
    } else {
        cwd
    };

    let mut updated = p.clone();
    updated.tabs.push(TabProfile {
        name,
        cwd,
        command,
        layout: None,
        panes: vec![],
    });

    let saved = ctx.muster.update_profile(updated)?;
    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&saved)?);
    } else {
        println!(
            "Added tab to {}: now {} tab(s)",
            saved.name,
            saved.tabs.len()
        );
    }
    Ok(())
}

pub(crate) fn execute_show(ctx: &CommandContext, id: &str) -> crate::error::Result {
    let p = resolve_profile(&ctx.muster, id)?;
    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&p)?);
    } else {
        println!("{} {} ({})", color_dot(&p.color), p.name, p.id);
        println!("  color: {}", p.color);
        for (i, tab) in p.tabs.iter().enumerate() {
            let cmd = tab
                .command
                .as_deref()
                .map_or(String::new(), |c| format!(" — {c}"));
            println!("  [{i}] {}: {}{cmd}", tab.name, tab.cwd);
            if !tab.panes.is_empty() {
                if let Some(ref layout) = tab.layout {
                    println!("      layout: {layout}");
                }
                for (pi, pane) in tab.panes.iter().enumerate() {
                    let pane_cwd = pane.cwd.as_deref().unwrap_or("(inherit)");
                    let pane_cmd = pane
                        .command
                        .as_deref()
                        .map_or(String::new(), |c| format!(" — {c}"));
                    println!("      pane {pi}: {pane_cwd}{pane_cmd}");
                }
            }
        }
    }
    Ok(())
}

/// The `profile edit` command uses an interactive retry loop with user prompts.
/// `process::exit` is appropriate here for user-initiated aborts since we're
/// in the middle of an interactive dialogue that can't be expressed as `Err`.
pub(crate) fn execute_edit(ctx: &CommandContext, id: &str) -> crate::error::Result {
    let p = resolve_profile(&ctx.muster, id)?;
    let old_id = p.id.clone();
    let editable = EditableProfile::from(&p);
    let toml_str = toml::to_string_pretty(&editable)?;

    let saved = loop {
        let mut tmp = tempfile::Builder::new().suffix(".toml").tempfile()?;
        tmp.write_all(toml_str.as_bytes())?;
        tmp.flush()?;

        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".to_string());

        let status = process::Command::new(&editor)
            .arg(tmp.path())
            .status()
            .map_err(|e| {
                crate::error::CliError::User(format!("failed to launch editor \"{editor}\": {e}"))
            })?;

        if !status.success() {
            bail!("Editor exited with non-zero status");
        }

        let content = std::fs::read_to_string(tmp.path())?;
        let parsed: EditableProfile = match toml::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Parse error: {e}");
                eprint!("Retry? [Y/n] ");
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if answer.trim().eq_ignore_ascii_case("n") {
                    bail!("Aborted.");
                }
                continue;
            }
        };

        let mut profile = parsed.into_profile();

        // Validate color
        match muster::session::theme::resolve_color(&profile.color) {
            Ok(c) => profile.color = c,
            Err(e) => {
                eprintln!("Invalid color: {e}");
                eprint!("Retry? [Y/n] ");
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if answer.trim().eq_ignore_ascii_case("n") {
                    bail!("Aborted.");
                }
                continue;
            }
        }

        if profile.tabs.is_empty() {
            eprintln!("Profile must have at least one tab.");
            eprint!("Retry? [Y/n] ");
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if answer.trim().eq_ignore_ascii_case("n") {
                bail!("Aborted.");
            }
            continue;
        }

        // Handle rename vs update
        let result = if profile.id == old_id {
            ctx.muster.update_profile(profile)?
        } else {
            if ctx.muster.resolve_session(&old_id).is_ok() {
                bail!(
                    "Cannot rename: session for \"{}\" is running. Kill it first.",
                    p.name
                );
            }
            ctx.muster.rename_profile(&old_id, profile)?
        };

        break result;
    };

    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&saved)?);
    } else {
        println!("Saved: {} ({})", saved.name, saved.id);
    }
    Ok(())
}

pub(crate) fn execute_update(
    ctx: &CommandContext,
    id: &str,
    name: Option<&str>,
    color: Option<&str>,
) -> crate::error::Result {
    if name.is_none() && color.is_none() {
        bail!("At least one of --name or --color is required.");
    }

    let mut p = resolve_profile(&ctx.muster, id)?;
    let old_id = p.id.clone();

    if let Some(new_color) = color {
        p.color = muster::session::theme::resolve_color(new_color)?;
    }

    let saved = if let Some(new_name) = name {
        let new_id = muster::config::profile::slugify(new_name);
        if new_id != old_id && ctx.muster.resolve_session(&old_id).is_ok() {
            bail!("Kill session for \"{}\" before renaming.", p.name);
        }
        p.name = new_name.to_string();
        p.id = new_id;
        if p.id == old_id {
            ctx.muster.update_profile(p)?
        } else {
            ctx.muster.rename_profile(&old_id, p)?
        }
    } else {
        ctx.muster.update_profile(p)?
    };

    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&saved)?);
    } else {
        println!("Updated: {} ({})", saved.name, saved.id);
    }
    Ok(())
}

pub(crate) fn execute_remove_tab(
    ctx: &CommandContext,
    profile: &str,
    tab: &str,
) -> crate::error::Result {
    let mut p = resolve_profile(&ctx.muster, profile)?;

    let idx = if let Ok(i) = tab.parse::<usize>() {
        if i >= p.tabs.len() {
            bail!(
                "Tab index {i} out of range (profile has {} tab(s)).",
                p.tabs.len()
            );
        }
        i
    } else if let Some(i) = p.tabs.iter().position(|t| t.name == tab) {
        i
    } else {
        bail!("Tab not found: {tab}");
    };

    if p.tabs.len() == 1 {
        bail!("Cannot remove the last tab from a profile.");
    }

    p.tabs.remove(idx);
    let saved = ctx.muster.update_profile(p)?;

    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&saved)?);
    } else {
        println!(
            "Removed tab from {}: now {} tab(s)",
            saved.name,
            saved.tabs.len()
        );
    }
    Ok(())
}
