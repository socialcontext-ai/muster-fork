use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process;

/// Find the tmux binary path (same one the library uses).
pub(crate) fn tmux_path() -> PathBuf {
    which::which("tmux").unwrap_or_else(|_| PathBuf::from("tmux"))
}

/// Detect the default terminal emulator for the current platform.
///
/// On macOS, returns "terminal" (Terminal.app).
/// On Linux, probes for common terminals in order of preference,
/// falling back to "xterm".
fn detect_terminal() -> &'static str {
    // Linux: check for common terminals on PATH.
    const LINUX_CANDIDATES: &[&str] = &[
        "ghostty",
        "kitty",
        "alacritty",
        "wezterm",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "x-terminal-emulator", // Debian/Ubuntu alternative system
        "xterm",
    ];

    if cfg!(target_os = "macos") {
        return "terminal";
    }

    for candidate in LINUX_CANDIDATES {
        if which::which(candidate).is_ok() {
            return candidate;
        }
    }

    "xterm"
}

/// Resolve the terminal emulator to use: explicit setting, or platform default.
pub(crate) fn resolve_terminal(settings: &muster::Settings) -> String {
    settings
        .terminal
        .clone()
        .unwrap_or_else(|| detect_terminal().to_string())
}

/// Open a new terminal window running `tmux attach -t <session>`.
///
/// Platform-aware: uses macOS `open` for .app bundles, `osascript` for
/// Terminal.app/iTerm2, and direct execution with `-e` for Linux terminals.
fn open_terminal_with_tmux(terminal: &str, session: &str) {
    let tmux = tmux_path();
    let tmux_str = tmux.to_string_lossy();

    if cfg!(target_os = "macos") {
        open_terminal_macos(terminal, session, &tmux_str);
    } else {
        open_terminal_linux(terminal, session, &tmux_str);
    }
}

/// macOS terminal launch strategies.
fn open_terminal_macos(terminal: &str, session: &str, tmux_str: &str) {
    match terminal {
        "ghostty" => {
            let cmd = format!("{tmux_str} attach -t {session}");
            let _ = process::Command::new("open")
                .args([
                    "-na",
                    "Ghostty.app",
                    "--args",
                    "--quit-after-last-window-closed=true",
                    &format!("--command={cmd}"),
                ])
                .status();
        }
        "alacritty" => {
            let _ = process::Command::new("open")
                .args([
                    "-na",
                    "Alacritty.app",
                    "--args",
                    "-e",
                    tmux_str,
                    "attach",
                    "-t",
                    session,
                ])
                .status();
        }
        "kitty" => {
            let _ = process::Command::new("open")
                .args([
                    "-na",
                    "Kitty.app",
                    "--args",
                    tmux_str,
                    "attach",
                    "-t",
                    session,
                ])
                .status();
        }
        "wezterm" => {
            let _ = process::Command::new("open")
                .args([
                    "-na",
                    "WezTerm.app",
                    "--args",
                    "start",
                    "--",
                    tmux_str,
                    "attach",
                    "-t",
                    session,
                ])
                .status();
        }
        _ => {
            // AppleScript fallback — works for Terminal.app, iTerm2, and others
            // that support the `do script` AppleScript command.
            let app = if terminal == "terminal" {
                "Terminal"
            } else if terminal == "iterm2" || terminal == "iterm" {
                "iTerm"
            } else {
                terminal
            };
            let cmd = format!("{tmux_str} attach -t {session}");
            let script = format!(
                "tell application \"{app}\"\n\
                     activate\n\
                     do script \"{cmd}\"\n\
                 end tell"
            );
            let _ = process::Command::new("osascript")
                .args(["-e", &script])
                .status();
        }
    }
}

/// Linux terminal launch strategies.
fn open_terminal_linux(terminal: &str, session: &str, tmux_str: &str) {
    let attach_cmd = format!("{tmux_str} attach -t {session}");

    match terminal {
        "ghostty" => {
            let _ = process::Command::new("ghostty")
                .args(["--quit-after-last-window-closed=true", "-e", &attach_cmd])
                .spawn();
        }
        "kitty" => {
            let _ = process::Command::new("kitty")
                .args([tmux_str, "attach", "-t", session])
                .spawn();
        }
        "alacritty" => {
            let _ = process::Command::new("alacritty")
                .args(["-e", tmux_str, "attach", "-t", session])
                .spawn();
        }
        "wezterm" => {
            let _ = process::Command::new("wezterm")
                .args(["start", "--", tmux_str, "attach", "-t", session])
                .spawn();
        }
        "gnome-terminal" => {
            let _ = process::Command::new("gnome-terminal")
                .args(["--", tmux_str, "attach", "-t", session])
                .spawn();
        }
        "konsole" => {
            let _ = process::Command::new("konsole")
                .args(["-e", tmux_str, "attach", "-t", session])
                .spawn();
        }
        "xfce4-terminal" => {
            let _ = process::Command::new("xfce4-terminal")
                .args(["-e", &attach_cmd])
                .spawn();
        }
        _ => {
            // Generic fallback: most terminals accept `-e command`
            let _ = process::Command::new(terminal)
                .args(["-e", tmux_str, "attach", "-t", session])
                .spawn();
        }
    }
}

/// Attach to a tmux session.
///
/// If already inside tmux (`$TMUX` set), opens a new terminal window with the
/// session attached instead of nesting. Otherwise replaces the current process
/// with `tmux attach-session`.
pub(crate) fn exec_tmux_attach(session: &str, settings: &muster::Settings) -> ! {
    if std::env::var_os("TMUX").is_some() {
        let terminal = resolve_terminal(settings);
        open_terminal_with_tmux(&terminal, session);
        process::exit(0);
    }

    let err = process::Command::new(tmux_path())
        .args(["attach-session", "-t", session])
        .env_remove("CLAUDECODE")
        .exec();
    // exec() only returns on error
    eprintln!("Failed to exec tmux: {err}");
    process::exit(1);
}

/// Send a notification, preferring native macOS desktop notifications when available.
///
/// On macOS (outside SSH), launches `MusterNotify.app` via `open` — this provides
/// native `UNUserNotificationCenter` banners with click-to-source navigation.
/// Falls back to tmux display-message if the helper isn't installed or fails.
pub(crate) fn send_notification(
    summary: &str,
    body: &str,
    session: &str,
    window: &str,
    terminal: &str,
) {
    if cfg!(target_os = "macos") && std::env::var_os("SSH_CONNECTION").is_none() {
        let app_dir = dirs::config_dir()
            .unwrap_or_default()
            .join("muster/MusterNotify.app");
        if app_dir.exists() {
            let spawned = process::Command::new("open")
                .args([
                    "-n",
                    app_dir.to_str().unwrap_or_default(),
                    "--args",
                    summary,
                    body,
                    "--session",
                    session,
                    "--window",
                    window,
                    "--terminal",
                    terminal,
                    "--timeout",
                    "30",
                ])
                .spawn();
            if spawned.is_ok() {
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
    let _ = process::Command::new(tmux_path())
        .args(["display-message", "-d", "5000", &msg])
        .status();
}

/// Install the MusterNotify.app notification helper bundle into ~/.config/muster/.
///
/// macOS requires a `CFBundleIdentifier` for persistent Notification Center access.
/// This creates a minimal .app bundle containing the `muster-notify` binary,
/// codesigns it, and prints instructions for first-run permission grant.
pub(crate) fn setup_notifications() -> crate::error::Result {
    let config_dir = dirs::config_dir()
        .ok_or("Could not determine config directory")?
        .join("muster");
    let bundle_dir = config_dir.join("MusterNotify.app");
    let app_dir = bundle_dir.join("Contents");
    let macos_dir = app_dir.join("MacOS");
    std::fs::create_dir_all(&macos_dir)?;

    // Write Info.plist
    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>com.muster.notifier</string>
  <key>CFBundleName</key>
  <string>MusterNotify</string>
  <key>CFBundleDisplayName</key>
  <string>Muster Notifications</string>
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
        crate::error::bail!(
            "Could not find muster-notify binary.\nInstall it: cargo install --path crates/muster-notify"
        );
    };

    let dest = macos_dir.join("muster-notify");
    std::fs::copy(&source, &dest)?;

    // Make sure it's executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
    }

    // Codesign the bundle (ad-hoc signature, required for notification permissions)
    let codesign_status = process::Command::new("codesign")
        .args([
            "--force",
            "--sign",
            "-",
            "--identifier",
            "com.muster.notifier",
            bundle_dir.to_str().unwrap_or_default(),
        ])
        .status();
    match codesign_status {
        Ok(s) if s.success() => println!("Bundle codesigned successfully."),
        Ok(s) => eprintln!("Warning: codesign exited with {s}"),
        Err(e) => eprintln!("Warning: codesign failed: {e}"),
    }

    println!("Notification app installed: {}", bundle_dir.display());
    println!();
    println!("To grant notification permission, run once:");
    println!("  open \"{}\"", bundle_dir.display());
    println!("macOS will prompt you to allow notifications from Muster Notifications.");

    Ok(())
}

/// Remove the MusterNotify.app bundle and clean up delivered notifications.
pub(crate) fn uninstall_notifications() -> crate::error::Result {
    let bundle_dir = dirs::config_dir()
        .ok_or("Could not determine config directory")?
        .join("muster/MusterNotify.app");

    if bundle_dir.exists() {
        std::fs::remove_dir_all(&bundle_dir)?;
        println!("Removed {}", bundle_dir.display());
    } else {
        println!("Nothing to remove (bundle not found).");
    }

    // Also remove the old Muster.app bundle if it exists
    let old_bundle = dirs::config_dir()
        .unwrap_or_default()
        .join("muster/Muster.app");
    if old_bundle.exists() {
        std::fs::remove_dir_all(&old_bundle)?;
        println!("Removed old {}", old_bundle.display());
    }

    Ok(())
}
