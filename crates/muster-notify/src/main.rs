//! muster-notify — Native macOS notifications with click-to-source navigation.
//!
//! Usage: `muster-notify <title> <body> [--session S] [--window W] [--terminal T] [--timeout N]`
//!
//! Sends a `UNUserNotificationCenter` notification with a "Go to source" action.
//! On click: tmux select-window → open terminal attached to the session.
//! Process exits after click or timeout.
//!
//! Flow is event-driven via `CFRunLoop`:
//!   `main()` → request auth → [run loop] → auth callback → send notification
//!   → [run loop] → click callback → `handle_click` → stop run loop → exit
//!
//! Requirements:
//!   - Must be in a .app bundle with matching Info.plist + codesign
//!   - Must be launched via `open MusterNotify.app --args ...`
//!   - `NSApplication` init required for permission dialog on first run

use std::cell::RefCell;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_core_foundation::CFRunLoop;
use objc2_foundation::{
    MainThreadMarker, NSArray, NSError, NSObject, NSObjectProtocol, NSSet, NSString, ns_string,
};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationAction,
    UNNotificationActionOptions, UNNotificationCategory, UNNotificationCategoryOptions,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

// ---------------------------------------------------------------------------
// Logging (stderr + file, since `open` detaches stderr)
// ---------------------------------------------------------------------------

const LOG_PATH: &str = "/tmp/muster-notify.log";

fn log(msg: &str) {
    eprintln!("{msg}");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(f, "{msg}");
    }
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

struct Args {
    title: String,
    body: String,
    session: Option<String>,
    window: Option<String>,
    terminal: String,
    timeout: u64,
}

impl Args {
    fn parse_from_env() -> Self {
        let raw: Vec<String> = env::args().skip(1).collect();

        let mut title = String::new();
        let mut body = String::new();
        let mut session = None;
        let mut window = None;
        let mut terminal = "terminal".to_string();
        let mut timeout = 30u64;
        let mut positional = 0;
        let mut i = 0;

        while i < raw.len() {
            match raw[i].as_str() {
                "--session" => {
                    i += 1;
                    session = raw.get(i).cloned();
                }
                "--window" => {
                    i += 1;
                    window = raw.get(i).cloned();
                }
                "--terminal" => {
                    i += 1;
                    if let Some(val) = raw.get(i) {
                        terminal.clone_from(val);
                    }
                }
                "--timeout" => {
                    i += 1;
                    if let Some(val) = raw.get(i) {
                        timeout = val.parse().unwrap_or(30);
                    }
                }
                _ => {
                    match positional {
                        0 => title.clone_from(&raw[i]),
                        1 => body.clone_from(&raw[i]),
                        _ => {}
                    }
                    positional += 1;
                }
            }
            i += 1;
        }

        // No args: send a test notification (used for first-run permission grant)
        if title.is_empty() {
            title = "Muster".to_string();
        }
        if body.is_empty() {
            body = "Notifications are working.".to_string();
        }

        Args {
            title,
            body,
            session,
            window,
            terminal,
            timeout,
        }
    }
}

// ---------------------------------------------------------------------------
// Notification delegate
// ---------------------------------------------------------------------------

struct DelegateIvars {
    session: RefCell<Option<String>>,
    window: RefCell<Option<String>>,
    terminal: RefCell<String>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements.
    #[unsafe(super = NSObject)]
    #[name = "MusterNotificationDelegate"]
    #[ivars = DelegateIvars]
    struct NotificationDelegate;

    unsafe impl NSObjectProtocol for NotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        // Show banner+sound even when the app bundle is "foreground".
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            handler.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::Sound,));
        }

        // User tapped notification content or the "Go to source" action button.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            handler: &DynBlock<dyn Fn()>,
        ) {
            let action_id = response.actionIdentifier().to_string();
            log(&format!("[muster-notify] action: {action_id}"));

            // Both the default tap and our custom action trigger navigation.
            // Only dismiss (swipe away) is ignored.
            let is_dismiss = action_id == "com.apple.UNNotificationDismissActionIdentifier";
            if !is_dismiss {
                let session = self.ivars().session.borrow().clone();
                let window = self.ivars().window.borrow().clone();
                let terminal = self.ivars().terminal.borrow().clone();
                handle_click(session.as_deref(), window.as_deref(), &terminal);
            }

            handler.call(());

            // Stop the run loop so the process can exit.
            if let Some(rl) = CFRunLoop::main() {
                rl.stop();
            }
        }
    }
);

impl NotificationDelegate {
    fn new(session: Option<String>, window: Option<String>, terminal: String) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars {
            session: RefCell::new(session),
            window: RefCell::new(window),
            terminal: RefCell::new(terminal),
        });
        unsafe { msg_send![super(this), init] }
    }
}

// ---------------------------------------------------------------------------
// Click handler — open terminal attached to tmux session
// ---------------------------------------------------------------------------

fn tmux_bin() -> String {
    which::which("tmux").map_or_else(
        |_| "/opt/homebrew/bin/tmux".into(),
        |p| p.to_string_lossy().to_string(),
    )
}

fn handle_click(session: Option<&str>, window: Option<&str>, terminal: &str) {
    // No session means this was a test notification — nothing to navigate to.
    let Some(sess) = session else { return };
    let tmux = tmux_bin();

    // Check if the session still exists.
    let session_exists = Command::new(&tmux)
        .args(["has-session", "-t", sess])
        .output()
        .is_ok_and(|o| o.status.success());

    if !session_exists {
        log(&format!(
            "[muster-notify] session {sess} no longer exists, skipping"
        ));
        return;
    }

    // Switch to the target window (best-effort).
    if let Some(win) = window {
        let target = format!("{sess}:={win}");
        log(&format!("[muster-notify] select-window -t {target}"));
        let _ = Command::new(&tmux)
            .args(["select-window", "-t", &target])
            .output();
    }

    // Open a new terminal window attached to the session.
    log(&format!(
        "[muster-notify] opening {terminal}: tmux attach -t {sess}"
    ));

    match terminal {
        "ghostty" => {
            let cmd = format!("{tmux} attach -t {sess}");
            let _ = Command::new("open")
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
            let _ = Command::new("open")
                .args([
                    "-na",
                    "Alacritty.app",
                    "--args",
                    "-e",
                    &tmux,
                    "attach",
                    "-t",
                    sess,
                ])
                .status();
        }
        "kitty" => {
            let _ = Command::new("open")
                .args(["-na", "Kitty.app", "--args", &tmux, "attach", "-t", sess])
                .status();
        }
        "wezterm" => {
            let _ = Command::new("open")
                .args([
                    "-na",
                    "WezTerm.app",
                    "--args",
                    "start",
                    "--",
                    &tmux,
                    "attach",
                    "-t",
                    sess,
                ])
                .status();
        }
        _ => {
            // AppleScript fallback — works for Terminal.app, iTerm2, etc.
            let app_name = if terminal == "terminal" {
                "Terminal"
            } else if terminal == "iterm2" || terminal == "iterm" {
                "iTerm"
            } else {
                terminal
            };
            let cmd = format!("{tmux} attach -t {sess}");
            let script = format!(
                "tell application \"{app_name}\"\n\
                     activate\n\
                     do script \"{cmd}\"\n\
                 end tell"
            );
            let _ = Command::new("osascript").args(["-e", &script]).status();
        }
    }
}

// ---------------------------------------------------------------------------
// Send notification
// ---------------------------------------------------------------------------

fn send_notification(center: &UNUserNotificationCenter, title: &str, body: &str, has_source: bool) {
    if has_source {
        let action = UNNotificationAction::actionWithIdentifier_title_options(
            ns_string!("GO_TO_SOURCE_ACTION"),
            ns_string!("Go to source"),
            UNNotificationActionOptions::Foreground,
        );
        let category =
            UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                ns_string!("MUSTER_GOTO_SOURCE"),
                &NSArray::from_retained_slice(&[action]),
                &NSArray::<NSString>::new(),
                UNNotificationCategoryOptions::CustomDismissAction,
            );
        center.setNotificationCategories(&NSSet::from_retained_slice(&[category]));
    }

    let content = UNMutableNotificationContent::new();
    let title_ns = NSString::from_str(title);
    let body_ns = NSString::from_str(body);
    content.setTitle(&title_ns);
    content.setBody(&body_ns);
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    if has_source {
        content.setCategoryIdentifier(ns_string!("MUSTER_GOTO_SOURCE"));
    }

    let req_id = NSString::from_str(&format!(
        "muster-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    let request =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&req_id, &content, None);

    let add_block = RcBlock::new(|error: *mut NSError| {
        if error.is_null() {
            log("[muster-notify] notification sent");
        } else {
            let err = unsafe { &*error };
            log(&format!("[muster-notify] failed to send: {err:?}"));
            if let Some(rl) = CFRunLoop::main() {
                rl.stop();
            }
        }
    });

    center.addNotificationRequest_withCompletionHandler(&request, Some(&add_block));
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let _ = std::fs::write(LOG_PATH, "");

    let args = Args::parse_from_env();
    log(&format!(
        "[muster-notify] title={:?} body={:?} session={:?} window={:?} terminal={:?} timeout={}",
        args.title, args.body, args.session, args.window, args.terminal, args.timeout
    ));

    // NSApplication init as accessory (no dock icon). Required for the
    // notification permission dialog to appear on first run.
    let mtm = MainThreadMarker::new().expect("must run on main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let center = UNUserNotificationCenter::currentNotificationCenter();

    // Delegate is forgotten (leaked) — process is short-lived.
    let delegate = NotificationDelegate::new(
        args.session.clone(),
        args.window.clone(),
        args.terminal.clone(),
    );
    center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    std::mem::forget(delegate);

    // Request authorization. On first run, macOS shows the permission dialog.
    // The callback chains into send_notification on success.
    let title = args.title.clone();
    let body = args.body.clone();
    let has_source = args.session.is_some();
    let auth_block = RcBlock::new(move |granted: Bool, error: *mut NSError| {
        let granted = granted.as_bool();
        log(&format!("[muster-notify] auth: granted={granted}"));
        if !error.is_null() {
            let err = unsafe { &*error };
            log(&format!("[muster-notify] auth error: {err:?}"));
        }

        if !granted {
            log("[muster-notify] not authorized — enable in System Settings > Notifications");
            if let Some(rl) = CFRunLoop::main() {
                rl.stop();
            }
            return;
        }

        let center = UNUserNotificationCenter::currentNotificationCenter();
        send_notification(&center, &title, &body, has_source);
    });

    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &auth_block,
    );

    // Timeout thread: stop run loop so process exits if no interaction.
    let timeout_secs = args.timeout;
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(timeout_secs));
        log(&format!("[muster-notify] timeout ({timeout_secs}s)"));
        if let Some(rl) = CFRunLoop::main() {
            rl.stop();
        }
    });

    log(&format!(
        "[muster-notify] waiting (timeout: {timeout_secs}s)..."
    ));
    CFRunLoop::run();
    log("[muster-notify] done");
}
