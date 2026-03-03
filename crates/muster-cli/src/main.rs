use std::io::IsTerminal;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use muster::{Muster, TabProfile};

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
        /// Tab definition (name:cwd[:command]), repeatable
        #[arg(long)]
        tab: Vec<String>,
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
        /// Tab definition (name:cwd[:command]), repeatable
        #[arg(long)]
        tab: Vec<String>,
        /// Color (hex)
        #[arg(long, default_value = "#808080")]
        color: String,
    },

    /// Add a tab to an existing profile
    AddTab {
        /// Profile name or ID
        profile: String,
        /// Tab name
        #[arg(long)]
        name: String,
        /// Working directory
        #[arg(long)]
        cwd: String,
        /// Startup command
        #[arg(long)]
        command: Option<String>,
    },
}

/// Parse a `name:cwd[:command]` string into a `TabProfile`.
fn parse_tab(input: &str) -> Result<TabProfile, String> {
    let parts: Vec<&str> = input.splitn(3, ':').collect();
    if parts.len() < 2 {
        return Err(format!(
            "invalid tab format '{input}': expected 'name:cwd' or 'name:cwd:command'"
        ));
    }
    let name = parts[0].to_string();
    let cwd = if parts[1] == "." {
        std::env::current_dir().map_or_else(|_| ".".to_string(), |p| p.to_string_lossy().to_string())
    } else {
        parts[1].to_string()
    };
    let command = parts
        .get(2)
        .map(std::string::ToString::to_string)
        .filter(|s| !s.is_empty());
    Ok(TabProfile { name, cwd, command })
}

/// Build tabs from `--tab` flags, defaulting to a single Shell tab at $HOME.
fn build_tabs(raw: &[String]) -> Result<Vec<TabProfile>, String> {
    if raw.is_empty() {
        let home = dirs::home_dir()
            .map_or_else(|| "/tmp".to_string(), |p| p.to_string_lossy().to_string());
        return Ok(vec![TabProfile {
            name: "Shell".to_string(),
            cwd: home,
            command: None,
        }]);
    }
    raw.iter().map(|s| parse_tab(s)).collect()
}

/// Render a colored dot using ANSI truecolor. Falls back to plain dot if not a TTY.
fn color_dot(hex: &str) -> String {
    if !std::io::stdout().is_terminal() {
        return "●".to_string();
    }
    if let Ok((r, g, b)) = muster::session::theme::hex_to_rgb(hex) {
        format!("\x1b[38;2;{r};{g};{b}m●\x1b[0m")
    } else {
        "●".to_string()
    }
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
                        println!("  {} {} ({}){}", color_dot(&p.color), p.name, p.id, marker);
                    }
                }
                if !sessions.is_empty() {
                    println!("\nSessions:");
                    for s in &sessions {
                        println!(
                            "  {} {} — {} ({} windows){}",
                            color_dot(&s.color),
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
            tab,
            color,
            detach,
        } => {
            let tabs = build_tabs(&tab)?;
            let color = muster::session::theme::resolve_color(&color)?;

            let profile = muster::Profile {
                id: muster::config::profile::slugify(&name),
                name: name.clone(),
                color,
                tabs,
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
            match m.resolve_session(&session) {
                Ok(session_name) => {
                    m.set_color(&session_name, &color)?;
                    if !cli.json {
                        println!("Color updated: {session_name} → {color}");
                    }
                }
                Err(_) => {
                    // No running session — try updating the profile directly
                    let profiles = m.list_profiles()?;
                    let found = profiles
                        .iter()
                        .find(|p| p.name == session || p.id == session);
                    let Some(p) = found else {
                        eprintln!("No session or profile found: {session}");
                        process::exit(1);
                    };
                    let resolved = muster::session::theme::resolve_color(&color)?;
                    let mut profile = p.clone();
                    profile.color = resolved;
                    m.update_profile(profile)?;
                    if !cli.json {
                        println!("Color updated: {} → {color}", p.name);
                    }
                }
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
                        "{} {} — {} [{} windows]",
                        color_dot(&s.color),
                        s.session_name,
                        s.display_name,
                        s.window_count,
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
                            "  {} {} ({}) — {} tab(s)",
                            color_dot(&p.color),
                            p.name,
                            p.id,
                            p.tabs.len(),
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

            ProfileAction::Save { name, tab, color } => {
                let tabs = build_tabs(&tab)?;
                let color = muster::session::theme::resolve_color(&color)?;

                let profile = muster::Profile {
                    id: muster::config::profile::slugify(&name),
                    name: name.clone(),
                    color,
                    tabs,
                };

                let saved = m.save_profile(profile)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&saved)?);
                } else {
                    println!("Saved: {} ({})", saved.name, saved.id);
                }
            }

            ProfileAction::AddTab {
                profile,
                name,
                cwd,
                command,
            } => {
                let profiles = m.list_profiles()?;
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
                });

                let saved = m.update_profile(updated)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&saved)?);
                } else {
                    println!(
                        "Added tab to {}: now {} tab(s)",
                        saved.name,
                        saved.tabs.len()
                    );
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
