use std::io::{IsTerminal, Write as _};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use muster::{Muster, Profile, TabProfile};

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

    /// Show processes running inside sessions
    Ps {
        /// Profile name or ID (shows all sessions if omitted)
        profile: Option<String>,
    },

    /// Show listening ports inside sessions
    Ports {
        /// Profile name or ID (shows all sessions if omitted)
        profile: Option<String>,
    },

    /// Show all sessions with details
    Status,

    /// Peek at recent terminal output
    Peek {
        /// Profile name, ID, or session name
        session: String,
        /// Window names to show (all if omitted)
        windows: Vec<String>,
        /// Lines of output per window
        #[arg(short = 'n', long, default_value = "50")]
        lines: u32,
    },

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

    /// Handle pane death notification (called by tmux hook)
    #[command(name = "_pane-died", hide = true)]
    PaneDied {
        session_name: String,
        window_name: String,
        pane_id: String,
        exit_code: i32,
    },

    /// Handle bell notification (called by tmux hook)
    #[command(name = "_bell", hide = true)]
    Bell {
        session_name: String,
        window_name: String,
    },

    /// Profile management
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Install macOS notification app bundle
    #[command(name = "setup-notifications")]
    SetupNotifications,
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

    /// Show a profile's full definition
    Show {
        /// Profile name or ID
        id: String,
    },

    /// Edit a profile in $EDITOR
    Edit {
        /// Profile name or ID
        id: String,
    },

    /// Update profile fields inline
    Update {
        /// Profile name or ID
        id: String,
        /// New display name
        #[arg(long)]
        name: Option<String>,
        /// New color (hex or named)
        #[arg(long)]
        color: Option<String>,
    },

    /// Remove a tab from a profile
    RemoveTab {
        /// Profile name or ID
        profile: String,
        /// Tab name or 0-based index
        tab: String,
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
    Ok(TabProfile {
        name,
        cwd,
        command,
        layout: None,
        panes: vec![],
    })
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
            layout: None,
            panes: vec![],
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

/// TOML representation of a profile for interactive editing.
/// Excludes `id` since it's derived from `name` via slugify.
#[derive(serde::Serialize, serde::Deserialize)]
struct EditableProfile {
    name: String,
    color: String,
    tabs: Vec<EditableTab>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EditableTab {
    name: String,
    cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layout: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    panes: Vec<EditablePane>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct EditablePane {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    command: Option<String>,
}

impl From<&Profile> for EditableProfile {
    fn from(p: &Profile) -> Self {
        Self {
            name: p.name.clone(),
            color: p.color.clone(),
            tabs: p
                .tabs
                .iter()
                .map(|t| EditableTab {
                    name: t.name.clone(),
                    cwd: t.cwd.clone(),
                    command: t.command.clone(),
                    layout: t.layout.clone(),
                    panes: t
                        .panes
                        .iter()
                        .map(|p| EditablePane {
                            cwd: p.cwd.clone(),
                            command: p.command.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl EditableProfile {
    fn into_profile(self) -> Profile {
        Profile {
            id: muster::config::profile::slugify(&self.name),
            name: self.name,
            color: self.color,
            tabs: self
                .tabs
                .into_iter()
                .map(|t| TabProfile {
                    name: t.name,
                    cwd: t.cwd,
                    command: t.command,
                    layout: t.layout,
                    panes: t
                        .panes
                        .into_iter()
                        .map(|p| muster::PaneProfile {
                            cwd: p.cwd,
                            command: p.command,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

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

fn default_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
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

/// Send a notification, preferring macOS desktop notifications when available.
///
/// On macOS (outside SSH), tries the Muster.app notification helper first —
/// this has a CFBundleIdentifier so macOS Notification Center works properly.
/// Falls back to tmux display-message if the helper isn't installed or fails.
fn send_notification(summary: &str, body: &str) {
    if cfg!(target_os = "macos") && std::env::var_os("SSH_CONNECTION").is_none() {
        let app_binary = dirs::config_dir()
            .unwrap_or_default()
            .join("muster/Muster.app/Contents/MacOS/muster-notify");
        if app_binary.exists() {
            let status = std::process::Command::new(&app_binary)
                .args([summary, body])
                .status();
            if status.is_ok_and(|s| s.success()) {
                return;
            }
        }
    }

    // Fallback: tmux display-message
    let msg = if body.is_empty() {
        summary.to_string()
    } else {
        format!("{summary} — {body}")
    };
    let _ = std::process::Command::new(tmux_path())
        .args(["display-message", "-d", "5000", &msg])
        .status();
}

/// Install the Muster.app notification helper bundle into ~/.config/muster/.
///
/// macOS requires a CFBundleIdentifier for persistent Notification Center access.
/// This creates a minimal .app bundle containing the `muster-notify` binary.
fn setup_notifications() -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = dirs::config_dir()
        .ok_or("Could not determine config directory")?
        .join("muster");
    let app_dir = config_dir.join("Muster.app/Contents");
    let macos_dir = app_dir.join("MacOS");
    std::fs::create_dir_all(&macos_dir)?;

    // Write Info.plist
    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>com.muster.notify</string>
  <key>CFBundleName</key>
  <string>Muster</string>
  <key>CFBundleExecutable</key>
  <string>muster-notify</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleVersion</key>
  <string>1.0</string>
  <key>LSUIElement</key>
  <true/>
</dict>
</plist>
"#;
    std::fs::write(app_dir.join("Info.plist"), plist)?;

    // Find muster-notify binary:
    // 1. Next to the running muster binary (e.g. both in ~/.cargo/bin/)
    // 2. On PATH
    let notify_binary = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            let sibling = exe.parent()?.join("muster-notify");
            sibling.exists().then_some(sibling)
        })
        .or_else(|| which::which("muster-notify").ok());

    let Some(source) = notify_binary else {
        eprintln!("Could not find muster-notify binary.");
        eprintln!("Install it: cargo install --path crates/muster-notify");
        std::process::exit(1);
    };

    let dest = macos_dir.join("muster-notify");
    std::fs::copy(&source, &dest)?;

    // Make sure it's executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
    }

    println!("Notification app installed: {}", config_dir.join("Muster.app").display());
    println!("macOS may prompt you to allow notifications from Muster.");

    Ok(())
}

// ---- Process tree support for `muster ps` ----

struct ProcessInfo {
    pid: u32,
    ppid: u32,
    command: String,
}

#[derive(serde::Serialize)]
struct ProcessTree {
    pid: u32,
    command: String,
    children: Vec<ProcessTree>,
}

/// Parse `ps -eo pid,ppid,comm` output into a process table.
fn parse_process_table(output: &str) -> Vec<ProcessInfo> {
    output
        .lines()
        .skip(1) // skip header
        .filter_map(|line| {
            let line = line.trim();
            let mut tokens = line.split_whitespace();
            let pid: u32 = tokens.next()?.parse().ok()?;
            let parent: u32 = tokens.next()?.parse().ok()?;
            // Rejoin the rest — command may contain spaces
            let command: String = tokens.collect::<Vec<_>>().join(" ");
            if command.is_empty() {
                return None;
            }
            Some(ProcessInfo { pid, ppid: parent, command })
        })
        .collect()
}

/// Run `ps -eo pid,ppid,comm` and parse the full process table.
fn build_process_table() -> Vec<ProcessInfo> {
    let output = match std::process::Command::new("ps")
        .args(["-eo", "pid,ppid,comm"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => return Vec::new(),
    };
    parse_process_table(&output)
}

/// Build a process tree rooted at `root_pid` from the system process table.
fn build_tree(root_pid: u32, table: &[ProcessInfo]) -> Vec<ProcessTree> {
    let children: Vec<&ProcessInfo> = table.iter().filter(|p| p.ppid == root_pid).collect();
    children
        .into_iter()
        .map(|child| ProcessTree {
            pid: child.pid,
            command: child.command.clone(),
            children: build_tree(child.pid, table),
        })
        .collect()
}

/// Render a process tree with box-drawing characters at a given indent level.
fn render_tree(tree: &[ProcessTree], prefix: &str) {
    for (i, node) in tree.iter().enumerate() {
        let is_last = i == tree.len() - 1;
        let connector = if is_last { "└─" } else { "├─" };
        println!(
            "{prefix}{connector} {} (PID {})",
            node.command, node.pid
        );
        let child_prefix = if is_last {
            format!("{prefix}   ")
        } else {
            format!("{prefix}│  ")
        };
        render_tree(&node.children, &child_prefix);
    }
}

// ---- Listening port support for `muster ports` ----

struct MatchedPort {
    port: u16,
    address: String,
    pid: u32,
    command: String,
    session_name: String,
    display_name: String,
    color: String,
    window_index: u32,
    window_name: String,
}

struct ListeningPort {
    pid: u32,
    port: u16,
    address: String,
    command: String,
}

/// Parse `lsof -i -P -n -sTCP:LISTEN` output into listening port entries.
fn parse_listening_ports(output: &str) -> Vec<ListeningPort> {
    output
        .lines()
        .skip(1) // skip header
        .filter_map(|line| {
            // Columns: COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 10 {
                return None;
            }
            let command = cols[0].to_string();
            let pid: u32 = cols[1].parse().ok()?;
            // NAME field is second-to-last: "*:8000 (LISTEN)" splits into
            // [..., "*:8000", "(LISTEN)"]
            let name = cols[cols.len() - 2];
            let (address, port_str) = name.rsplit_once(':')?;
            let port: u16 = port_str.parse().ok()?;
            Some(ListeningPort {
                pid,
                port,
                address: address.to_string(),
                command,
            })
        })
        .collect()
}

/// Run `lsof -i -P -n -sTCP:LISTEN` and parse all listening TCP ports.
/// Returns `None` if lsof is unavailable or fails, `Some(vec)` on success.
fn build_listening_ports() -> Option<Vec<ListeningPort>> {
    let output = match std::process::Command::new("lsof")
        .args(["-i", "-P", "-n", "-sTCP:LISTEN"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        // lsof exits with 1 when there are no results — still a successful run
        Ok(o) if o.status.code() == Some(1) => return Some(Vec::new()),
        _ => return None,
    };
    Some(parse_listening_ports(&output))
}

/// Recursively collect all PIDs from a process tree.
fn collect_pids(tree: &[ProcessTree]) -> Vec<u32> {
    let mut pids = Vec::new();
    for node in tree {
        pids.push(node.pid);
        pids.extend(collect_pids(&node.children));
    }
    pids
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
            if let Ok(session_name) = m.resolve_session(&session) {
                m.set_color(&session_name, &color)?;
                if !cli.json {
                    println!("Color updated: {session_name} → {color}");
                }
            } else {
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

        Command::Ps { profile } => {
            let mut sessions = m.list_sessions()?;

            // Filter to matching profile if specified
            if let Some(ref filter) = profile {
                sessions.retain(|s| {
                    s.display_name == *filter
                        || s.profile_id.as_deref() == Some(filter)
                        || s.session_name == *filter
                });
                if sessions.is_empty() {
                    eprintln!("No session found for: {filter}");
                    process::exit(1);
                }
            }

            if sessions.is_empty() {
                if cli.json {
                    println!("[]");
                } else {
                    println!("No active sessions.");
                }
            } else {
                let proc_table = build_process_table();

                if cli.json {
                    let mut json_sessions = Vec::new();
                    for s in &sessions {
                        let panes = m.client().list_panes(&s.session_name).unwrap_or_default();
                        // Group panes by window
                        let mut window_map: std::collections::BTreeMap<u32, Vec<&muster::TmuxPane>> =
                            std::collections::BTreeMap::new();
                        for pane in &panes {
                            window_map.entry(pane.window_index).or_default().push(pane);
                        }
                        let windows = m.client().list_windows(&s.session_name).unwrap_or_default();
                        let json_windows: Vec<serde_json::Value> = windows
                            .iter()
                            .map(|w| {
                                let w_panes = window_map.get(&w.index).cloned().unwrap_or_default();
                                let json_panes: Vec<serde_json::Value> = w_panes
                                    .iter()
                                    .map(|p| {
                                        let children = build_tree(p.pid, &proc_table);
                                        serde_json::json!({
                                            "index": p.index,
                                            "pid": p.pid,
                                            "command": p.command,
                                            "cwd": p.cwd,
                                            "children": children,
                                        })
                                    })
                                    .collect();
                                serde_json::json!({
                                    "index": w.index,
                                    "name": w.name,
                                    "cwd": w.cwd,
                                    "panes": json_panes,
                                })
                            })
                            .collect();

                        json_sessions.push(serde_json::json!({
                            "session": s.session_name,
                            "display_name": s.display_name,
                            "color": s.color,
                            "windows": json_windows,
                        }));
                    }
                    println!("{}", serde_json::to_string_pretty(&json_sessions)?);
                } else {
                    for s in &sessions {
                        let panes = m.client().list_panes(&s.session_name).unwrap_or_default();
                        let windows = m.client().list_windows(&s.session_name).unwrap_or_default();

                        println!(
                            "{} {} ({}) [{} windows]",
                            color_dot(&s.color),
                            s.display_name,
                            s.session_name,
                            s.window_count,
                        );

                        // Group panes by window index
                        let mut pane_map: std::collections::BTreeMap<u32, Vec<&muster::TmuxPane>> =
                            std::collections::BTreeMap::new();
                        for pane in &panes {
                            pane_map.entry(pane.window_index).or_default().push(pane);
                        }

                        for w in &windows {
                            println!("  [{}] {} {}", w.index, w.name, w.cwd);
                            if let Some(w_panes) = pane_map.get(&w.index) {
                                for pane in w_panes {
                                    println!(
                                        "      {} (PID {})",
                                        pane.command, pane.pid
                                    );
                                    let children = build_tree(pane.pid, &proc_table);
                                    render_tree(&children, "        ");
                                }
                            }
                        }
                    }
                }
            }
        }

        Command::Ports { profile } => {
            let mut sessions = m.list_sessions()?;

            if let Some(ref filter) = profile {
                sessions.retain(|s| {
                    s.display_name == *filter
                        || s.profile_id.as_deref() == Some(filter)
                        || s.session_name == *filter
                });
                if sessions.is_empty() {
                    eprintln!("No session found for: {filter}");
                    process::exit(1);
                }
            }

            if sessions.is_empty() {
                if cli.json {
                    println!("[]");
                } else {
                    println!("No active sessions.");
                }
            } else {
                let Some(listening) = build_listening_ports() else {
                    eprintln!("Could not query listening ports: lsof not found or failed.");
                    process::exit(1);
                };
                if listening.is_empty() {
                    if cli.json {
                        println!("[]");
                    } else {
                        println!("No listening ports found in muster sessions.");
                    }
                } else {
                    let proc_table = build_process_table();

                    // Build a PID -> (session, window_index, window_name) lookup
                    // across all sessions
                    let mut pid_lookup: std::collections::HashMap<
                        u32,
                        (String, String, String, u32, String),
                    > = std::collections::HashMap::new();

                    for s in &sessions {
                        let panes = m.client().list_panes(&s.session_name).unwrap_or_default();
                        let windows = m.client().list_windows(&s.session_name).unwrap_or_default();

                        // Map window index -> name
                        let window_names: std::collections::HashMap<u32, String> = windows
                            .iter()
                            .map(|w| (w.index, w.name.clone()))
                            .collect();

                        for pane in &panes {
                            // Build tree from pane PID and collect all descendant PIDs
                            let tree = build_tree(pane.pid, &proc_table);
                            let mut all_pids = vec![pane.pid];
                            all_pids.extend(collect_pids(&tree));

                            let window_name = window_names
                                .get(&pane.window_index)
                                .cloned()
                                .unwrap_or_default();

                            for pid in all_pids {
                                pid_lookup.entry(pid).or_insert_with(|| {
                                    (
                                        s.session_name.clone(),
                                        s.display_name.clone(),
                                        s.color.clone(),
                                        pane.window_index,
                                        window_name.clone(),
                                    )
                                });
                            }
                        }
                    }

                    // Match listening ports to sessions
                    let mut matched: Vec<MatchedPort> = listening
                        .iter()
                        .filter_map(|lp| {
                            pid_lookup.get(&lp.pid).map(
                                |(session_name, display_name, color, window_index, window_name)| {
                                    MatchedPort {
                                        port: lp.port,
                                        address: lp.address.clone(),
                                        pid: lp.pid,
                                        command: lp.command.clone(),
                                        session_name: session_name.clone(),
                                        display_name: display_name.clone(),
                                        color: color.clone(),
                                        window_index: *window_index,
                                        window_name: window_name.clone(),
                                    }
                                },
                            )
                        })
                        .collect();

                    if matched.is_empty() {
                        if cli.json {
                            println!("[]");
                        } else {
                            println!("No listening ports found in muster sessions.");
                        }
                    } else if cli.json {
                        let json_ports: Vec<serde_json::Value> = matched
                            .iter()
                            .map(|mp| {
                                serde_json::json!({
                                    "port": mp.port,
                                    "address": mp.address,
                                    "pid": mp.pid,
                                    "command": mp.command,
                                    "session": mp.session_name,
                                    "display_name": mp.display_name,
                                    "color": mp.color,
                                    "window_index": mp.window_index,
                                    "window_name": mp.window_name,
                                })
                            })
                            .collect();
                        println!("{}", serde_json::to_string_pretty(&json_ports)?);
                    } else {
                        // Group by session, sort ports within each group
                        matched.sort_by(|a, b| {
                            a.session_name
                                .cmp(&b.session_name)
                                .then(a.port.cmp(&b.port))
                        });

                        let mut current_session = String::new();
                        for mp in &matched {
                            if mp.session_name != current_session {
                                if !current_session.is_empty() {
                                    println!();
                                }
                                println!(
                                    "{} {} ({})",
                                    color_dot(&mp.color),
                                    mp.display_name,
                                    mp.session_name,
                                );
                                current_session.clone_from(&mp.session_name);
                            }
                            println!(
                                "  :{:<6} {:<16} [{}] {}",
                                mp.port, mp.command, mp.window_index, mp.window_name,
                            );
                        }
                    }
                }
            }
        }

        Command::PaneDied {
            session_name,
            window_name,
            pane_id,
            exit_code,
        } => {
            let display_name = m
                .client()
                .get_option(&session_name, "@muster_name")?
                .unwrap_or_else(|| session_name.clone());

            // Capture last output from the dying pane before kill
            let snapshot = m.client().capture_pane(&pane_id, 50).unwrap_or_default();

            // Save snapshot to logs directory
            let log_dir = config_dir.join("logs").join(&session_name);
            let _ = std::fs::create_dir_all(&log_dir);
            let log_file = log_dir.join(format!("{window_name}.last"));
            let _ = std::fs::write(&log_file, &snapshot);

            // Include last few lines in notification body
            let last_lines: String = snapshot
                .lines()
                .rev()
                .take(5)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let body = if last_lines.is_empty() {
                format!("Exit code: {exit_code}")
            } else {
                format!("Exit code: {exit_code}\n{last_lines}")
            };

            let summary = format!("Exited: {display_name} \u{25b8} {window_name}");
            send_notification(&summary, &body);

            // Clean up the dead pane
            let _ = m.client().cmd(&["kill-pane", "-t", &pane_id]);
        }

        Command::Bell {
            session_name,
            window_name,
        } => {
            let display_name = m
                .client()
                .get_option(&session_name, "@muster_name")?
                .unwrap_or_else(|| session_name.clone());

            let summary = format!("Bell: {display_name} \u{25b8} {window_name}");

            send_notification(&summary, "");
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
                            let stale = m
                                .client()
                                .get_window_option(
                                    &s.session_name,
                                    w.index,
                                    "@muster_layout_stale",
                                )
                                .ok()
                                .flatten()
                                .is_some();
                            let stale_tag = if stale {
                                if std::io::stdout().is_terminal() {
                                    " \x1b[33;1m(layout unsaved)\x1b[0m"
                                } else {
                                    " (layout unsaved)"
                                }
                            } else {
                                ""
                            };
                            println!(
                                "  {marker} {}: {} ({}){stale_tag}",
                                w.index, w.name, w.cwd
                            );
                        }
                    }
                }
            }
        }

        Command::Peek {
            session,
            windows,
            lines,
        } => {
            let session_name = m.resolve_session(&session)?;
            let all_windows = m.client().list_windows(&session_name)?;

            let targets: Vec<_> = if windows.is_empty() {
                all_windows.iter().collect()
            } else {
                all_windows
                    .iter()
                    .filter(|w| {
                        windows
                            .iter()
                            .any(|name| w.name.eq_ignore_ascii_case(name))
                    })
                    .collect()
            };

            if targets.is_empty() {
                eprintln!("No matching windows found.");
                process::exit(1);
            }

            if cli.json {
                let entries: Vec<serde_json::Value> = targets
                    .iter()
                    .map(|win| {
                        let target = format!("{}:{}", session_name, win.index);
                        let output = m
                            .client()
                            .capture_pane(&target, lines)
                            .unwrap_or_default();
                        serde_json::json!({
                            "window": win.name,
                            "index": win.index,
                            "output": output.trim_end(),
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                for (i, win) in targets.iter().enumerate() {
                    if i > 0 {
                        println!();
                    }
                    let header = format!("\u{2500}\u{2500} {} ", win.name);
                    let pad = 40usize.saturating_sub(header.len());
                    println!("{}{}", header, "\u{2500}".repeat(pad));
                    let target = format!("{}:{}", session_name, win.index);
                    match m.client().capture_pane(&target, lines) {
                        Ok(output) => {
                            let trimmed = output.trim_end();
                            if trimmed.is_empty() {
                                println!("(empty)");
                            } else {
                                println!("{trimmed}");
                            }
                        }
                        Err(e) => eprintln!("  (capture failed: {e})"),
                    }
                }
            }
        }

        Command::Pin => {
            let result = m.pin_window()?;
            if !cli.json {
                match result {
                    muster::PinResult::Pinned => println!("Window pinned to profile."),
                    muster::PinResult::LayoutUpdated => println!("Layout saved to profile."),
                    muster::PinResult::AlreadyCurrent => println!("Layout already up to date."),
                }
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

        Command::SetupNotifications => {
            setup_notifications()?;
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
                    layout: None,
                    panes: vec![],
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

            ProfileAction::Show { id } => {
                let p = resolve_profile(&m, &id)?;
                if cli.json {
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
                                let pane_cwd = pane
                                    .cwd
                                    .as_deref()
                                    .unwrap_or("(inherit)");
                                let pane_cmd = pane
                                    .command
                                    .as_deref()
                                    .map_or(String::new(), |c| format!(" — {c}"));
                                println!("      pane {pi}: {pane_cwd}{pane_cmd}");
                            }
                        }
                    }
                }
            }

            ProfileAction::Edit { id } => {
                let p = resolve_profile(&m, &id)?;
                let old_id = p.id.clone();
                let editable = EditableProfile::from(&p);
                let toml_str = toml::to_string_pretty(&editable)?;

                let saved = loop {
                    let mut tmp = tempfile::Builder::new()
                        .suffix(".toml")
                        .tempfile()?;
                    tmp.write_all(toml_str.as_bytes())?;
                    tmp.flush()?;

                    let editor = std::env::var("EDITOR")
                        .or_else(|_| std::env::var("VISUAL"))
                        .unwrap_or_else(|_| "vi".to_string());

                    let status = process::Command::new(&editor)
                        .arg(tmp.path())
                        .status()?;

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
                        m.update_profile(profile)?
                    } else {
                        // Check for active session on old ID
                        if m.resolve_session(&old_id).is_ok() {
                            eprintln!(
                                "Cannot rename: session for \"{}\" is running. Kill it first.",
                                p.name
                            );
                            process::exit(1);
                        }
                        m.rename_profile(&old_id, profile)?
                    };

                    break result;
                };

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&saved)?);
                } else {
                    println!("Saved: {} ({})", saved.name, saved.id);
                }
            }

            ProfileAction::Update { id, name, color } => {
                if name.is_none() && color.is_none() {
                    eprintln!("At least one of --name or --color is required.");
                    process::exit(1);
                }

                let mut p = resolve_profile(&m, &id)?;
                let old_id = p.id.clone();

                if let Some(ref new_color) = color {
                    p.color = muster::session::theme::resolve_color(new_color)?;
                }

                let saved = if let Some(ref new_name) = name {
                    let new_id = muster::config::profile::slugify(new_name);
                    if new_id != old_id {
                        // Check for active session on old ID
                        if m.resolve_session(&old_id).is_ok() {
                            eprintln!(
                                "Kill session for \"{}\" before renaming.",
                                p.name
                            );
                            process::exit(1);
                        }
                    }
                    p.name.clone_from(new_name);
                    p.id = new_id;
                    if p.id == old_id {
                        m.update_profile(p)?
                    } else {
                        m.rename_profile(&old_id, p)?
                    }
                } else {
                    m.update_profile(p)?
                };

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&saved)?);
                } else {
                    println!("Updated: {} ({})", saved.name, saved.id);
                }
            }

            ProfileAction::RemoveTab { profile, tab } => {
                let mut p = resolve_profile(&m, &profile)?;

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
                let saved = m.update_profile(p)?;

                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&saved)?);
                } else {
                    println!(
                        "Removed tab from {}: now {} tab(s)",
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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_process_table tests ----

    #[test]
    fn parse_ps_basic() {
        let output = "  PID  PPID COMM\n    1     0 /sbin/launchd\n  100     1 /usr/sbin/syslogd\n  200   100 /usr/bin/some_daemon\n";
        let table = parse_process_table(output);
        assert_eq!(table.len(), 3);
        assert_eq!(table[0].pid, 1);
        assert_eq!(table[0].ppid, 0);
        assert_eq!(table[0].command, "/sbin/launchd");
        assert_eq!(table[1].pid, 100);
        assert_eq!(table[1].ppid, 1);
        assert_eq!(table[2].pid, 200);
        assert_eq!(table[2].ppid, 100);
    }

    #[test]
    fn parse_ps_empty_output() {
        let output = "  PID  PPID COMM\n";
        let table = parse_process_table(output);
        assert!(table.is_empty());
    }

    #[test]
    fn parse_ps_command_with_spaces() {
        let output = "  PID  PPID COMM\n  500   100 /usr/local/bin/my tool\n";
        let table = parse_process_table(output);
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].command, "/usr/local/bin/my tool");
    }

    #[test]
    fn parse_ps_skips_malformed_lines() {
        let output = "  PID  PPID COMM\n  notapid  1 /bin/sh\n  100     1 /usr/bin/daemon\n  abc   def ghi\n";
        let table = parse_process_table(output);
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].pid, 100);
    }

    // ---- build_tree tests ----

    fn sample_process_table() -> Vec<ProcessInfo> {
        // Tree structure:
        //   1 (init)
        //   ├─ 10 (fish)
        //   │  ├─ 100 (npm)
        //   │  │  └─ 101 (node)
        //   │  └─ 102 (cargo)
        //   └─ 20 (bash)
        vec![
            ProcessInfo { pid: 10, ppid: 1, command: "fish".into() },
            ProcessInfo { pid: 20, ppid: 1, command: "bash".into() },
            ProcessInfo { pid: 100, ppid: 10, command: "npm".into() },
            ProcessInfo { pid: 101, ppid: 100, command: "node".into() },
            ProcessInfo { pid: 102, ppid: 10, command: "cargo".into() },
        ]
    }

    #[test]
    fn build_tree_from_root() {
        let table = sample_process_table();
        let tree = build_tree(1, &table);
        assert_eq!(tree.len(), 2); // fish and bash
        assert_eq!(tree[0].command, "fish");
        assert_eq!(tree[0].children.len(), 2); // npm and cargo
        assert_eq!(tree[1].command, "bash");
        assert!(tree[1].children.is_empty());
    }

    #[test]
    fn build_tree_from_subtree() {
        let table = sample_process_table();
        let tree = build_tree(10, &table);
        assert_eq!(tree.len(), 2); // npm, cargo
        let npm = &tree[0];
        assert_eq!(npm.command, "npm");
        assert_eq!(npm.children.len(), 1);
        assert_eq!(npm.children[0].command, "node");
    }

    #[test]
    fn build_tree_leaf_node() {
        let table = sample_process_table();
        let tree = build_tree(101, &table);
        assert!(tree.is_empty()); // node has no children
    }

    #[test]
    fn build_tree_nonexistent_root() {
        let table = sample_process_table();
        let tree = build_tree(9999, &table);
        assert!(tree.is_empty());
    }

    // ---- collect_pids tests ----

    #[test]
    fn collect_pids_full_tree() {
        let table = sample_process_table();
        let tree = build_tree(1, &table);
        let mut pids = collect_pids(&tree);
        pids.sort();
        assert_eq!(pids, vec![10, 20, 100, 101, 102]);
    }

    #[test]
    fn collect_pids_subtree() {
        let table = sample_process_table();
        let tree = build_tree(10, &table);
        let mut pids = collect_pids(&tree);
        pids.sort();
        assert_eq!(pids, vec![100, 101, 102]);
    }

    #[test]
    fn collect_pids_empty_tree() {
        let pids = collect_pids(&[]);
        assert!(pids.is_empty());
    }

    // ---- parse_listening_ports tests ----

    #[test]
    fn parse_lsof_basic() {
        let output = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
python3.1 12345  usr    4u  IPv4 0xabcdef1234567890      0t0  TCP *:8000 (LISTEN)
node      23456  usr   21u  IPv6 0x1234567890abcdef      0t0  TCP [::1]:5173 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 2);

        assert_eq!(ports[0].command, "python3.1");
        assert_eq!(ports[0].pid, 12345);
        assert_eq!(ports[0].port, 8000);
        assert_eq!(ports[0].address, "*");

        assert_eq!(ports[1].command, "node");
        assert_eq!(ports[1].pid, 23456);
        assert_eq!(ports[1].port, 5173);
        assert_eq!(ports[1].address, "[::1]");
    }

    #[test]
    fn parse_lsof_localhost() {
        let output = "\
COMMAND   PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
Obsidian 9999  usr   36u  IPv4 0xaabbccdd11223344      0t0  TCP 127.0.0.1:27124 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].address, "127.0.0.1");
        assert_eq!(ports[0].port, 27124);
    }

    #[test]
    fn parse_lsof_empty_output() {
        let output = "COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME\n";
        let ports = parse_listening_ports(output);
        assert!(ports.is_empty());
    }

    #[test]
    fn parse_lsof_skips_short_lines() {
        let output = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
short line
python3   12345  usr    4u  IPv4 0xabcdef1234567890      0t0  TCP *:9000 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 9000);
    }

    #[test]
    fn parse_lsof_ipv6_wildcard() {
        let output = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
node      11111  usr   19u  IPv6 0xdeadbeef12345678      0t0  TCP *:3000 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].address, "*");
        assert_eq!(ports[0].port, 3000);
    }

    #[test]
    fn parse_lsof_multiple_ports_same_process() {
        let output = "\
COMMAND     PID USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
node      11111  usr   19u  IPv4 0xaaaa000000000001      0t0  TCP *:3000 (LISTEN)
node      11111  usr   20u  IPv6 0xaaaa000000000002      0t0  TCP *:3000 (LISTEN)
node      11111  usr   21u  IPv4 0xaaaa000000000003      0t0  TCP 127.0.0.1:3001 (LISTEN)
";
        let ports = parse_listening_ports(output);
        assert_eq!(ports.len(), 3);
        // All same PID
        assert!(ports.iter().all(|p| p.pid == 11111));
        assert_eq!(ports[0].port, 3000);
        assert_eq!(ports[2].port, 3001);
        assert_eq!(ports[2].address, "127.0.0.1");
    }

    // ---- parse_tab tests ----

    #[test]
    fn parse_tab_name_and_cwd() {
        let tab = parse_tab("Shell:/home/user").unwrap();
        assert_eq!(tab.name, "Shell");
        assert_eq!(tab.cwd, "/home/user");
        assert!(tab.command.is_none());
    }

    #[test]
    fn parse_tab_with_command() {
        let tab = parse_tab("Dev:/home/user:npm run dev").unwrap();
        assert_eq!(tab.name, "Dev");
        assert_eq!(tab.cwd, "/home/user");
        assert_eq!(tab.command.as_deref(), Some("npm run dev"));
    }

    #[test]
    fn parse_tab_empty_command_becomes_none() {
        let tab = parse_tab("Shell:/home/user:").unwrap();
        assert!(tab.command.is_none());
    }

    #[test]
    fn parse_tab_missing_cwd_fails() {
        assert!(parse_tab("Shell").is_err());
    }
}
