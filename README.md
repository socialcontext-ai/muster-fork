# Muster

Terminal session group management built on tmux. A Rust library and CLI for organizing terminal sessions into named, color-coded groups with saved profiles, runtime theming, and push-based state synchronization via tmux control mode.

## Prerequisites

- **tmux** — installed and available in PATH
- **Rust** — 2021 edition (for building from source)

## Installation

```bash
cargo install --path crates/muster-cli
cargo install --path crates/muster-notify  # optional: macOS desktop notifications
```

## Quick Start

```bash
# Save a profile for a project
muster profile save "PKM" --tab 'Shell:~/work/pkm' --color '#f97316'

# Save a multi-tab profile
muster profile save "Web App" --color orange \
  --tab 'Shell:~/work/app' \
  --tab 'Server:~/work/app:npm start' \
  --tab 'Logs:~/var/log'

# Launch it — creates the tmux session and drops you in
muster launch "PKM"
# You're now inside a tmux session. Detach with Ctrl-b d to get back to your shell.

# From another terminal, check what's running
muster status

# Reattach to a running session by profile name
muster launch "PKM"

# Or attach by session name directly
muster attach muster_pkm

# Create a quick throwaway session (defaults to a Shell tab at $HOME)
muster new "Scratch"
# Again, you're immediately inside it.

# Create without attaching (background)
muster launch "PKM" --detach
muster new "Background" --detach

# Add a tab to an existing profile
muster profile add-tab "PKM" --name Editor --cwd ~/work/pkm
```

### Typical Workflow

1. **`muster profile save`** — define a project (name, tabs, color)
2. **`muster launch <name>`** — start or reattach (execs `tmux attach`, replacing your shell)
3. Work inside tmux. Use `Ctrl-b d` to detach back to your regular shell.
4. **`muster launch <name>`** again to reattach later
5. **`muster status`** from another terminal to see all sessions
6. **`muster kill <session>`** when done

`launch` is idempotent — if the session already exists, it attaches. If not, it creates from the profile and attaches.

## CLI Reference

```
muster launch <profile-name-or-id> [--detach]  # Create/attach to a session (default: attaches)
muster attach <session-name> [--window N]      # Attach to a running session
muster new <name> [--tab 'name:cwd[:cmd]' ...] [--color hex] [--detach]
muster kill <session-name>                     # Destroy a session
muster list                                    # List profiles and running sessions
muster status                                  # Show sessions with window details
muster color <session> <hex-color>             # Change session color live
muster pin                                     # Pin current tmux window to session profile
muster unpin                                   # Unpin current tmux window from profile
muster profile save <name> [--tab 'name:cwd[:cmd]' ...] [--color hex]
muster profile show <name-or-id>
muster profile edit <name-or-id>              # Open in $EDITOR as TOML
muster profile update <name-or-id> [--name ...] [--color ...]
muster profile add-tab <profile> --name <name> --cwd <dir> [--command <cmd>]
muster profile remove-tab <profile> <tab>     # By name or 0-based index
muster profile list
muster profile delete <name-or-id>
muster setup-notifications                  # Install macOS desktop notification support
```

The `--tab` flag uses colon-delimited format: `name:cwd` or `name:cwd:command`. It is repeatable for multiple tabs. If omitted, defaults to a single "Shell" tab at `$HOME`.

`launch`, `attach`, and `new` replace the current process with `tmux attach` (via exec). Use `--detach` to create without attaching. `--json` is available on all commands for machine-readable output. `--config-dir` or `MUSTER_CONFIG_DIR` overrides the default config directory (`~/.config/muster/`).

## Concepts

**Terminal group** — A named tmux session containing one or more windows (tabs), each with a working directory and optional startup command. Groups have a display name, color, and optional profile reference.

**Profile** — A saved template for creating a group. Stored in `~/.config/muster/profiles.json`. Contains the group's name, color, and tab definitions.

**Session** — A running tmux session managed by muster. Session names are prefixed with `muster_` to distinguish them from personal tmux sessions. Application metadata (name, color, profile ID) is stored as tmux user options (`@muster_name`, `@muster_color`, `@muster_profile`) on the session itself — no separate state file.

## Architecture

Muster is organized as a Cargo workspace with three crates:

```
crates/
├── muster/         # Library — tmux bindings, profiles, theming, control mode
├── muster-cli/     # CLI binary
└── muster-notify/  # macOS notification helper (minimal binary for Muster.app bundle)
```

### Library Modules

| Module | Purpose |
|--------|---------|
| `tmux::client` | Command execution, output parsing, session/window CRUD |
| `tmux::control` | Control mode connection, event stream parsing (`MusterEvent`) |
| `tmux::types` | `TmuxSession`, `TmuxWindow`, `SessionInfo` |
| `config::profile` | Profile CRUD with atomic JSON persistence |
| `config::settings` | Settings (tmux path, shell preference) |
| `session` | Session lifecycle — create from profile, destroy |
| `session::theme` | Hex color parsing, dimming, tmux status bar styling |
| `muster` | `Muster` facade tying everything together |

### Library Usage

```rust
use muster::{Muster, Profile, TabProfile};
use std::path::Path;

let m = Muster::init(Path::new("~/.config/muster"))?;

// Create a profile
let profile = Profile {
    id: "my-project".into(),
    name: "My Project".into(),
    color: "#f97316".into(),
    tabs: vec![
        TabProfile { name: "Shell".into(), cwd: "/home/user/project".into(), command: None },
        TabProfile { name: "Server".into(), cwd: "/home/user/project".into(), command: Some("npm run dev".into()) },
    ],
};
m.save_profile(profile.clone())?;

// Launch a session
let info = m.launch(&profile.id)?;

// List running sessions
let sessions = m.list_sessions()?;

// Subscribe to events (for GUI integration)
let rx = m.subscribe();
```

### Sources of Truth

| Source | Owns |
|--------|------|
| **tmux** | All running state: windows, CWDs, active window, plus `@muster_*` metadata |
| **Config directory** | Saved profiles and settings — never runtime state |

There is no application-level cache. When you need session state, you ask tmux.

### Control Mode

For long-running consumers (GUI applications), muster can establish tmux control mode connections that provide push-based notifications:

- `%window-add`, `%window-close`, `%window-renamed` — tab lifecycle
- `%session-window-changed` — active tab changes
- `%sessions-changed` — session lifecycle
- Response block framing (`%begin`/`%end`) for command output

These are parsed into `MusterEvent` variants and distributed via `tokio::broadcast`.

## Configuration

### `~/.config/muster/profiles.json`

```json
{
  "profiles": {
    "pkm-project": {
      "id": "pkm-project",
      "name": "PKM Project",
      "color": "#f97316",
      "tabs": [
        { "name": "Shell", "cwd": "/Users/sbb/work/pkm", "command": null },
        { "name": "Server", "cwd": "/Users/sbb/work/pkm", "command": "npm run dev" }
      ]
    }
  }
}
```

### `~/.config/muster/settings.json`

```json
{
  "tmux_path": null,
  "shell": "/usr/local/bin/fish"
}
```

`shell` overrides the default shell for new tmux panes. If omitted, muster uses `$SHELL`. Set this if your `$SHELL` differs from the shell you actually use (common on macOS where `$SHELL` defaults to `/bin/zsh`). `tmux_path` overrides tmux discovery from `$PATH`.

## Notifications

Muster sends notifications on session events — pane exits and terminal bell alerts. By default these appear as tmux status bar messages.

On macOS, you can enable native desktop notifications (Notification Center) by installing the notification helper:

```bash
cargo install --path crates/muster-notify
muster setup-notifications
```

This creates a minimal `Muster.app` bundle at `~/.config/muster/Muster.app/` containing the `muster-notify` helper binary. The app bundle provides a `CFBundleIdentifier` (`com.muster.notify`) that macOS requires for persistent Notification Center access. macOS may prompt you to allow notifications from Muster on first use.

When the helper is installed, notifications are delivered to Notification Center instead of the tmux status bar. Over SSH (`SSH_CONNECTION` is set), muster falls back to tmux display-message automatically.

## Testing

```bash
# Unit tests (no tmux required)
cargo nextest run

# All tests including integration (requires tmux)
cargo nextest run --run-ignored all

# Or with cargo test
cargo test                          # unit tests
cargo test -- --ignored             # integration tests
```

Integration tests create sessions with unique names and clean up after themselves. They do not interfere with your personal tmux sessions.

## Development

```bash
cargo t              # alias for cargo nextest run
cargo clippy         # lint
cargo fmt --check    # format check
```
