use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use muster::Muster;

#[derive(Parser)]
#[command(name = "muster", version, about = "Terminal session group management")]
struct Cli {
    /// Path to the config directory
    #[arg(long, env = "MUSTER_CONFIG_DIR")]
    config_dir: Option<PathBuf>,

    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List profiles and running sessions
    List,

    /// Launch or attach to a profile's session
    Launch {
        /// Profile name or ID
        profile: String,
        /// Create session but don't attach
        #[arg(long)]
        detach: bool,
    },

    /// Attach to a running session
    Attach {
        /// Profile name, ID, or session name
        session: String,
        /// Window index to switch to
        #[arg(long)]
        window: Option<u32>,
    },

    /// Destroy a session
    Kill {
        /// Profile name, ID, or session name
        session: String,
    },

    /// Create an ad-hoc session
    New {
        /// Display name
        name: String,
        /// Working directory
        #[arg(long, default_value = ".")]
        cwd: String,
        /// Color (hex)
        #[arg(long, default_value = "#808080")]
        color: String,
        /// Create session but don't attach
        #[arg(long)]
        detach: bool,
    },

    /// Change session color live
    Color {
        /// Profile name, ID, or session name
        session: String,
        /// Hex color (e.g. #f97316)
        color: String,
    },

    /// Show all sessions with details
    Status,

    /// Pin the current window to the session's profile
    Pin,

    /// Unpin the current window from the session's profile
    Unpin,

    /// Sync a window rename to the profile (called by tmux hook)
    #[command(hide = true)]
    SyncRename {
        /// Session name
        session: String,
        /// Window index
        window: u32,
        /// New window name
        name: String,
    },

    /// Profile management
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// List all profiles
    List,

    /// Delete a profile
    Delete {
        /// Profile name or ID
        id: String,
    },

    /// Save a new profile
    Save {
        /// Profile name
        name: String,
        /// Working directory for the first tab
        #[arg(long, default_value = ".")]
        cwd: String,
        /// Color (hex)
        #[arg(long, default_value = "#808080")]
        color: String,
    },
}

fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("muster")
}

/// Find the tmux binary path (same one the library uses).
fn tmux_path() -> PathBuf {
    which::which("tmux").unwrap_or_else(|_| PathBuf::from("tmux"))
}

/// Replace the current process with `tmux attach -t <session>`.
/// This never returns on success.
fn exec_tmux_attach(session: &str) -> ! {
    let err = std::process::Command::new(tmux_path())
        .args(["attach-session", "-t", session])
        .exec();
    // exec() only returns on error
    eprintln!("Failed to exec tmux: {err}");
    process::exit(1);
}

#[allow(clippy::too_many_lines)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config_dir = cli.config_dir.unwrap_or_else(default_config_dir);
    let m = Muster::init(&config_dir)?;

    match cli.command {
        Command::List => {
            let profiles = m.list_profiles()?;
            let sessions = m.list_sessions()?;

            if cli.json {
                let output = serde_json::json!({
                    "profiles": profiles,
                    "sessions": sessions,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                if !profiles.is_empty() {
                    println!("Profiles:");
                    for p in &profiles {
                        let active = sessions
                            .iter()
                            .any(|s| s.profile_id.as_deref() == Some(&p.id));
                        let marker = if active { " [active]" } else { "" };
                        println!("  {} ({}){}", p.name, p.id, marker);
                    }
                }
                if !sessions.is_empty() {
                    println!("\nSessions:");
                    for s in &sessions {
                        println!(
                            "  {} — {} ({} windows){}",
                            s.session_name,
                            s.display_name,
                            s.window_count,
                            if s.attached { " [attached]" } else { "" }
                        );
                    }
                }
                if profiles.is_empty() && sessions.is_empty() {
                    println!("No profiles or sessions.");
                }
            }
        }

        Command::Launch { profile, detach } => {
            let profiles = m.list_profiles()?;
            let found = profiles
                .iter()
                .find(|p| p.name == profile || p.id == profile);

            let Some(p) = found else {
                eprintln!("Profile not found: {profile}");
                process::exit(1);
            };
            let profile_id = p.id.clone();

            let info = m.launch(&profile_id)?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else if detach {
                println!("Launched: {} ({})", info.display_name, info.session_name);
            } else {
                // Replace this process with tmux attach
                exec_tmux_attach(&info.session_name);
            }
        }

        Command::Attach { session, window } => {
            let session_name = m.resolve_session(&session)?;

            if let Some(idx) = window {
                m.switch_window(&session_name, idx)?;
            }

            exec_tmux_attach(&session_name);
        }

        Command::Kill { session } => {
            let session = m.resolve_session(&session)?;
            m.destroy(&session)?;
            if !cli.json {
                println!("Destroyed: {session}");
            }
        }

        Command::New {
            name,
            cwd,
            color,
            detach,
        } => {
            let cwd = if cwd == "." {
                std::env::current_dir()?.to_string_lossy().to_string()
            } else {
                cwd
            };
            let color = muster::session::theme::resolve_color(&color)?;

            let profile = muster::Profile {
                id: muster::config::profile::slugify(&name),
                name: name.clone(),
                color,
                tabs: vec![muster::TabProfile {
                    name: "Shell".to_string(),
                    cwd,
                    command: None,
                }],
            };

            m.save_profile(profile.clone())?;
            let info = m.launch(&profile.id)?;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else if detach {
                println!("Created: {} ({})", info.display_name, info.session_name);
            } else {
                exec_tmux_attach(&info.session_name);
            }
        }

        Command::Color { session, color } => {
            let session = m.resolve_session(&session)?;
            m.set_color(&session, &color)?;
            if !cli.json {
                println!("Color updated: {session} → {color}");
            }
        }

        Command::Status => {
            let sessions = m.list_sessions()?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&sessions)?);
            } else if sessions.is_empty() {
                println!("No active sessions.");
            } else {
                for s in &sessions {
                    println!(
                        "{} — {} [{} windows] {}",
                        s.session_name, s.display_name, s.window_count, s.color
                    );
                    if let Ok(windows) = m.client().list_windows(&s.session_name) {
                        for w in &windows {
                            let marker = if w.active { "→" } else { " " };
                            println!("  {marker} {}: {} ({})", w.index, w.name, w.cwd);
                        }
                    }
                }
            }
        }

        Command::Pin => {
            m.pin_window()?;
            if !cli.json {
                println!("Window pinned to profile.");
            }
        }

        Command::Unpin => {
            m.unpin_window()?;
            if !cli.json {
                println!("Window unpinned from profile.");
            }
        }

        Command::SyncRename {
            session,
            window,
            name,
        } => {
            m.sync_rename(&session, window, &name)?;
        }

        Command::Profile { action } => match action {
            ProfileAction::List => {
                let profiles = m.list_profiles()?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&profiles)?);
                } else if profiles.is_empty() {
                    println!("No profiles.");
                } else {
                    for p in &profiles {
                        println!(
                            "  {} ({}) — {} tab(s), color: {}",
                            p.name,
                            p.id,
                            p.tabs.len(),
                            p.color
                        );
                    }
                }
            }

            ProfileAction::Delete { id } => {
                let profiles = m.list_profiles()?;
                let found = profiles.iter().find(|p| p.name == id || p.id == id);

                if let Some(p) = found {
                    let name = p.name.clone();
                    m.delete_profile(&p.id)?;
                    if !cli.json {
                        println!("Deleted: {name}");
                    }
                } else {
                    eprintln!("Profile not found: {id}");
                    process::exit(1);
                }
            }

            ProfileAction::Save { name, cwd, color } => {
                let cwd = if cwd == "." {
                    std::env::current_dir()?.to_string_lossy().to_string()
                } else {
                    cwd
                };
                let color = muster::session::theme::resolve_color(&color)?;

                let profile = muster::Profile {
                    id: muster::config::profile::slugify(&name),
                    name: name.clone(),
                    color,
                    tabs: vec![muster::TabProfile {
                        name: "Shell".to_string(),
                        cwd,
                        command: None,
                    }],
                };

                let saved = m.save_profile(profile)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&saved)?);
                } else {
                    println!("Saved: {} ({})", saved.name, saved.id);
                }
            }
        },
    }

    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
