use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process;

/// Find the tmux binary path (same one the library uses).
pub(crate) fn tmux_path() -> PathBuf {
    which::which("tmux").unwrap_or_else(|_| PathBuf::from("tmux"))
}

/// Attach to a tmux session.
///
/// If already inside tmux (`$TMUX` set), opens a new terminal window with the
/// session attached instead of nesting. Otherwise replaces the current process
/// with `tmux attach-session`.
pub(crate) fn exec_tmux_attach(session: &str, settings: &muster::Settings) -> ! {
    if std::env::var_os("TMUX").is_some() {
        // Inside tmux — open a new terminal window instead of nesting.
        let terminal = settings.terminal.as_deref().unwrap_or("ghostty");
        let tmux = tmux_path();
        let tmux_str = tmux.to_string_lossy();

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
                        &*tmux_str,
                        "attach",
                        "-t",
                        session,
                    ])
                    .status();
            }
            _ => {
                let app = if terminal == "terminal" {
                    "Terminal"
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
