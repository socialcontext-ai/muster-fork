use std::io::Write as _;
use std::process;

use muster::{Muster, Profile, TabProfile};

use super::CommandContext;
use crate::editing::EditableProfile;
use crate::format::color_dot;
use crate::tabs::build_tabs;

/// Resolve a profile by name or ID, exiting on failure.
fn resolve_profile(m: &Muster, input: &str) -> Result<Profile, Box<dyn std::error::Error>> {
    let profiles = m.list_profiles()?;
    let found = profiles
        .into_iter()
        .find(|p| p.name == input || p.id == input);
    if let Some(p) = found {
        Ok(p)
    } else {
        eprintln!("Profile not found: {input}");
        process::exit(1);
    }
}

pub(crate) fn execute_list(ctx: &CommandContext) -> Result<(), Box<dyn std::error::Error>> {
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

pub(crate) fn execute_delete(
    ctx: &CommandContext,
    id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let profiles = ctx.muster.list_profiles()?;
    let found = profiles.iter().find(|p| p.name == id || p.id == id);

    if let Some(p) = found {
        let name = p.name.clone();
        ctx.muster.delete_profile(&p.id)?;
        if !ctx.json {
            println!("Deleted: {name}");
        }
    } else {
        eprintln!("Profile not found: {id}");
        process::exit(1);
    }
    Ok(())
}

pub(crate) fn execute_save(
    ctx: &CommandContext,
    name: &str,
    tab: &[String],
    color: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let tabs = build_tabs(tab)?;
    let color = muster::session::theme::resolve_color(color)?;

    let profile = muster::Profile {
        id: muster::config::profile::slugify(name),
        name: name.to_string(),
        color,
        tabs,
    };

    let saved = ctx.muster.save_profile(profile)?;
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
) -> Result<(), Box<dyn std::error::Error>> {
    let profiles = ctx.muster.list_profiles()?;
    let found = profiles
        .iter()
        .find(|p| p.name == profile || p.id == profile);

    let Some(p) = found else {
        eprintln!("Profile not found: {profile}");
        process::exit(1);
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

pub(crate) fn execute_show(
    ctx: &CommandContext,
    id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub(crate) fn execute_edit(
    ctx: &CommandContext,
    id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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

        let status = process::Command::new(&editor).arg(tmp.path()).status()?;

        if !status.success() {
            eprintln!("Editor exited with non-zero status");
            process::exit(1);
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
                    eprintln!("Aborted.");
                    process::exit(1);
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
                    eprintln!("Aborted.");
                    process::exit(1);
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
                eprintln!("Aborted.");
                process::exit(1);
            }
            continue;
        }

        // Handle rename vs update
        let result = if profile.id == old_id {
            ctx.muster.update_profile(profile)?
        } else {
            if ctx.muster.resolve_session(&old_id).is_ok() {
                eprintln!(
                    "Cannot rename: session for \"{}\" is running. Kill it first.",
                    p.name
                );
                process::exit(1);
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
) -> Result<(), Box<dyn std::error::Error>> {
    if name.is_none() && color.is_none() {
        eprintln!("At least one of --name or --color is required.");
        process::exit(1);
    }

    let mut p = resolve_profile(&ctx.muster, id)?;
    let old_id = p.id.clone();

    if let Some(new_color) = color {
        p.color = muster::session::theme::resolve_color(new_color)?;
    }

    let saved = if let Some(new_name) = name {
        let new_id = muster::config::profile::slugify(new_name);
        if new_id != old_id && ctx.muster.resolve_session(&old_id).is_ok() {
            eprintln!("Kill session for \"{}\" before renaming.", p.name);
            process::exit(1);
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
) -> Result<(), Box<dyn std::error::Error>> {
    let mut p = resolve_profile(&ctx.muster, profile)?;

    let idx = if let Ok(i) = tab.parse::<usize>() {
        if i >= p.tabs.len() {
            eprintln!(
                "Tab index {i} out of range (profile has {} tab(s)).",
                p.tabs.len()
            );
            process::exit(1);
        }
        i
    } else if let Some(i) = p.tabs.iter().position(|t| t.name == tab) {
        i
    } else {
        eprintln!("Tab not found: {tab}");
        process::exit(1);
    };

    if p.tabs.len() == 1 {
        eprintln!("Cannot remove the last tab from a profile.");
        process::exit(1);
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
