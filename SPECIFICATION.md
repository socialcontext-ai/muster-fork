# Terminal Management Subsystem Specification

## 1. Overview

### 1.1 Problem Statement

Users working across multiple projects maintain numerous terminal sessions — development servers, test runners, Claude Code instances, build watchers — spread across many windows and tabs with no organizational structure. Finding the right terminal requires hunting through a dozen identical-looking windows. Context switching between projects is slow and disorienting.

### 1.2 Solution

A terminal group management layer that associates persistent terminal sessions with filesystem locations (projects). Two interfaces — a GUI integrated into the markdown browser application and a standalone CLI — provide complementary access to the same underlying system. The user organizes terminals into named, color-coded groups that map to project directories. Beacons in the file browser show which directories have active terminals. Launching, switching, and managing sessions is fast from either interface.

### 1.3 Core Concept

A **terminal group** is a named tmux session containing one or more windows (tabs), each associated with a working directory and optional startup command. Groups have metadata (name, color, profile reference) managed by the application, while runtime state (which tabs exist, their CWDs, window order) is owned by tmux.

The system is a dual-mode (GUI + CLI) tmux interface layer. tmux is the runtime. A shared config directory is the metadata store. The GUI and CLI are peers — both consumers of these two sources of truth.

---

## 2. Architecture

### 2.1 Library vs. Application

The terminal management core is implemented as a **standalone Rust library crate** (`muster` or similar), analogous to ParavaneFS. This library:

- Provides first-class **Rust tmux bindings** — command execution, output parsing, and control mode event streaming, with no external tmux library dependency
- Manages terminal group profiles (CRUD on saved configurations)
- Interfaces with tmux (session lifecycle, control mode connections, state observation)
- Applies themes and styling to tmux sessions
- Abstracts the terminal emulator layer (Ghostty today, extensible to others)
- Provides a testable API with no dependency on Tauri or any GUI framework

The tmux bindings are written in-house rather than depending on an external crate. The only existing Rust tmux library (`tmux_interface`, 64 stars) is self-described as experimental, has control mode listed as unimplemented, and wraps all 90 tmux commands when we need ~20. More importantly, the novel value of this library — control mode event streaming — is precisely what no existing Rust crate provides. The bindings are part of the product, not an implementation detail.

Consumers of this library:

| Consumer | Role |
|----------|------|
| **CLI binary** (`muster` or subcommand) | Standalone terminal management from the shell |
| **Tauri application** | GUI integration — beacons, group launcher, settings |
| **Tests** | Unit and integration tests against the library API |

This separation means the terminal management system can be developed, tested, and used independently of the Tauri application.

### 2.2 Source of Truth

There are exactly two sources of truth:

| Source | Owns | Accessed by |
|--------|------|-------------|
| **tmux** | All running session state: windows, CWDs, active window, plus application metadata stored as user options (`@muster_name`, `@muster_color`, `@muster_profile`) | Library reads via control mode + CLI commands |
| **Config directory** | Saved profiles (templates for creating sessions) and tool settings (emulator preference, paths) | Library reads/writes JSON files |

There is no application-level cache of tmux state. There is no runtime state file. When the GUI or CLI needs to know what tabs a group has, it asks tmux. When it needs the group's color, it asks tmux. The config directory stores only user-authored configuration (profiles, settings), never derived or runtime state.

### 2.3 Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri Application                     │
│  ┌──────────┐ ┌──────────────┐ ┌─────────────────────┐  │
│  │   File   │ │    Group     │ │    Search            │  │
│  │ Browser  │ │  Launcher UI │ │  (ParavaneFS)        │  │
│  │ (beacons)│ │              │ │                      │  │
│  └────┬─────┘ └──────┬───────┘ └──────────────────────┘  │
│       │              │                                    │
│       └──────┬───────┘                                    │
│              │                                            │
│         ┌────▼────┐                                       │
│         │ Tauri   │                                       │
│         │Commands │                                       │
│         └────┬────┘                                       │
└──────────────┼────────────────────────────────────────────┘
               │
        ┌──────▼──────┐         ┌──────────────┐
        │             │         │              │
        │   muster    │◄────────│  CLI binary  │
        │  (library)  │         │   (muster)   │
        │             │         │              │
        └──┬───────┬──┘         └──────────────┘
           │       │
     ┌─────▼──┐ ┌──▼──────────────┐
     │ Config │ │      tmux       │
     │  dir   │ │  (sessions,     │
     │ (JSON) │ │   control mode) │
     └────────┘ └────────┬────────┘
                         │
                  ┌──────▼───────┐
                  │   Ghostty    │
                  │  (rendering) │
                  └──────────────┘
```

### 2.4 Data Flow Principles

1. **Commands flow down**: GUI/CLI → library → tmux / config
2. **State is read from source**: runtime state always from tmux, metadata always from config files
3. **Events flow up**: tmux control mode pushes events → library → GUI (via Tauri events)
4. **No polling for state that tmux pushes**: window lifecycle, name changes, active window changes all come via control mode notifications
5. **CWD tracking via subscription or on-demand query**: `%subscription-changed` may provide push-based CWD tracking (to be verified). Fallback is on-demand queries when the UI needs beacon data. Even in the worst case, polling is limited to a single lightweight query per session — fundamentally different from the previous architecture where polling was the entire state sync mechanism.
6. **Control mode is the only push mechanism**: tmux hooks (`set-hook`) can trigger shell commands on events, but that requires building a custom IPC channel to receive those commands — effectively reinventing control mode with worse ergonomics. Control mode provides a structured, well-documented notification protocol directly over a persistent connection.

---

## 3. Config and State Management

### 3.1 Config Directory Layout

```
~/.config/muster/
├── profiles.json             # Saved terminal group profiles
└── settings.json             # Global settings (emulator preference, paths, etc.)
```

That's it. No runtime state file. Running session metadata (name, color, profile reference) is stored as tmux user options on the session itself (see Section 4.6). When a session dies, its runtime metadata dies with it — the profile retains the original values.

The config directory is owned by the library but the path is provided by the consumer at initialization. The CLI defaults to `~/.config/muster/`. The GUI app points the library at the same path, keeping its own non-terminal config (feeds, UI preferences) in a separate directory. This makes CLI and GUI true peers sharing the same profiles.

### 3.2 Profile Schema (`profiles.json`)

A profile is a template for creating a terminal group. It does not represent a running session.

```json
{
  "profiles": {
    "profile_<id>": {
      "id": "profile_<id>",
      "name": "PKM Project",
      "color": "#f97316",
      "tabs": [
        {
          "name": "Shell",
          "cwd": "/Users/sbb/work/pkm",
          "command": null
        },
        {
          "name": "Server",
          "cwd": "/Users/sbb/work/pkm/tauri-md-browser",
          "command": "npm run tauri dev"
        }
      ]
    }
  }
}
```

### 3.3 Running Session Metadata

Running session metadata is stored as **tmux user options** on the session itself, not in a separate file. tmux supports arbitrary user-defined options with the `@` prefix, which persist for the lifetime of the session and are queryable via `show-option` or format strings.

| tmux User Option | Value | Example |
|-----------------|-------|---------|
| `@muster_name` | Display name | `"PKM Project"` |
| `@muster_color` | Hex color | `"#f97316"` |
| `@muster_profile` | Profile ID (if launched from profile) | `"profile_abc123"` |

Set on session creation:
```bash
tmux set-option -t muster_abc123 @muster_name "PKM Project"
tmux set-option -t muster_abc123 @muster_color "#f97316"
tmux set-option -t muster_abc123 @muster_profile "profile_abc123"
```

Queried at any time:
```bash
tmux show-option -t muster_abc123 -v @muster_color
# or via format strings:
tmux list-sessions -F '#{session_name} #{@muster_name} #{@muster_color}'
```

This means tmux is the single source of truth for **all** running session state — both the runtime state it tracks natively (windows, CWDs, active window) and the application metadata we attach via user options. No separate state file, no reconciliation of file vs. tmux state, no concurrent write problems.

### 3.4 Settings Schema (`settings.json`)

```json
{
  "emulator": "ghostty",
  "emulator_path": null,
  "tmux_path": null
}
```

### 3.5 State Reconciliation

On library initialization (app startup or CLI invocation):

1. Query tmux for all `muster_*` sessions with their `@muster_*` user options
2. That's it — tmux is the source of truth, so there's nothing to reconcile against a file

If a session exists in tmux with the `muster_` prefix but is missing `@muster_*` options (e.g., created manually by the user, or options were cleared), the library treats it as an orphan and assigns defaults (name derived from session name, default color).

For long-running library consumers (the GUI app), control mode connections established after initialization provide ongoing push-based state updates. Short-lived consumers (CLI commands) just query tmux directly — no persistent connection needed for one-shot operations.

---

## 4. tmux Interface

### 4.1 Session Naming Convention

All managed sessions use the prefix `muster_` followed by the profile ID:
```
muster_profile_abc123
```

This allows the library to distinguish managed sessions from the user's personal tmux sessions.

### 4.2 Control Mode Connection

The library maintains one tmux control mode connection per active session. Control mode is a persistent stdin/stdout pipe opened via `tmux -CC attach -t <session_name>`. This is the **only** real push mechanism tmux offers — the alternative (tmux hooks via `set-hook`) can run shell commands on events, but that requires building a custom IPC channel to receive those commands (a Unix socket, named pipe, or similar), which amounts to reinventing control mode with worse ergonomics. Control mode gives us a structured, well-documented notification protocol directly.

Each connection:

- Is started with `tmux -CC attach -t <session_name>`
- Immediately sends `refresh-client -f no-output` to suppress all pane output (`%output` notifications)
- Receives push notifications for window/session lifecycle events
- Can send commands through stdin and receive structured responses
- Is dropped when the session is destroyed

**Response framing:** Commands sent through control mode produce structured output blocks:
```
%begin <timestamp> <command_number> <flags>
<output lines>
%end <timestamp> <command_number> <flags>
```
Or `%error` instead of `%end` on failure. The command number allows correlating responses to requests when multiple commands are in flight.

**Notifications** arrive outside of response blocks and are never interleaved with command output. This makes the stream parseable: lines starting with `%begin`/`%end`/`%error` are response framing, other `%`-prefixed lines are notifications.

### 4.3 Events Consumed

tmux control mode defines the following notifications. The library consumes the subset relevant to session group management:

**Window lifecycle (primary use case):**

| Notification | Payload | Library Action |
|-------------|---------|----------------|
| `%window-add <window-id>` | Window ID | Query window details, emit tab-added event |
| `%window-close <window-id>` | Window ID | Emit tab-closed event |
| `%window-renamed <window-id> <name>` | Window ID, new name | Emit tab-renamed event |
| `%session-window-changed <session-id> <window-id>` | Session ID, window ID | Emit active-tab-changed event |
| `%layout-change <window-id> <layout> <visible-layout> <flags>` | Layout details | Emit layout-changed if pane splits are tracked |

**Session lifecycle:**

| Notification | Payload | Library Action |
|-------------|---------|----------------|
| `%sessions-changed` | (none) | Re-query session list, emit updates |
| `%session-changed <session-id> <name>` | Session ID, name | Update active session tracking |
| `%session-renamed <name>` | New name | Update session metadata |
| `%client-detached <client>` | Client name | Track emulator disconnect |

**Subscriptions (for CWD tracking — see Section 9.2):**

| Notification | Payload | Library Action |
|-------------|---------|----------------|
| `%subscription-changed <name> <session-id> <window-id> <window-index> <pane-id> ... : <value>` | Format value | Emit CWD-changed event if tracking pane paths |

**Suppressed (via `refresh-client -f no-output`):**

| Notification | Why Suppressed |
|-------------|---------------|
| `%output <pane-id> <value>` | Terminal output — user sees this in the emulator directly |
| `%extended-output` | Same, extended form |

**Ignored (not relevant to session management):**

`%pane-mode-changed`, `%paste-buffer-changed`, `%paste-buffer-deleted`, `%continue`, `%pause`, `%config-error`, `%unlinked-window-*`, `%client-session-changed`

### 4.4 Commands Issued

| Operation | tmux Command |
|-----------|-------------|
| Create session from profile | `tmux new-session -d -s <name> -n <tab_name> -c <cwd>` then `new-window` per additional tab |
| Destroy session | `tmux kill-session -t <name>` |
| List sessions | `tmux list-sessions -F '#{session_name}'` |
| Query windows | `tmux list-windows -t <session> -F '#{window_index} #{window_name} #{pane_current_path}'` |
| Switch window | `tmux select-window -t <session>:<index>` |
| Add window | `tmux new-window -t <session> -n <name> -c <cwd>` |
| Close window | `tmux kill-window -t <session>:<index>` |
| Apply color theme | `tmux set-option -t <session> status-style "bg=<color>"` (see Section 6) |
| Query active window | `tmux display-message -t <session> -p '#{window_index}'` |

### 4.5 Session Lifecycle

**Creating a group from a profile:**
1. `tmux new-session -d -s muster_<profile_id> -n <tab_0_name> -c <tab_0_cwd>`
2. For each additional tab: `tmux new-window -t muster_<profile_id> -n <name> -c <cwd>`
3. For tabs with startup commands: `tmux send-keys -t muster_<profile_id>:<index> '<command>' Enter`
4. Set user options: `@muster_name`, `@muster_color`, `@muster_profile` (see Section 3.3)
5. Apply color theme (Section 6)
6. Open control mode connection
7. Launch emulator attached to the session

**Attaching to an existing session:**
1. Verify session exists via `tmux has-session -t <name>`
2. Launch emulator with `tmux attach -t <name>`
3. Optionally switch to a specific window: `tmux select-window -t <name>:<index>`

**Destroying a group:**
1. `tmux kill-session -t <name>` (user options are destroyed with the session)
2. Drop control mode connection
3. Emulator window closes automatically (tmux session gone)

---

## 5. Terminal Emulator Interface

### 5.1 Abstraction Layer

The library defines an `Emulator` trait for launching and managing terminal emulator windows:

```rust
pub trait Emulator: Send + Sync {
    /// Launch the emulator attached to a tmux session.
    /// Returns a handle or identifier for the spawned process.
    fn launch(&self, session_name: &str) -> Result<EmulatorHandle, Error>;

    /// Check if an emulator window is already open for this session.
    fn is_running(&self, session_name: &str) -> Result<bool, Error>;

    /// Focus an existing emulator window for this session (if supported).
    fn focus(&self, session_name: &str) -> Result<(), Error>;

    /// Return the command and args needed to attach to a session.
    fn attach_command(&self, session_name: &str) -> Vec<String>;
}
```

### 5.2 Ghostty Implementation

**Launching a new window:**
```bash
open -na Ghostty.app --args -e tmux attach -t <session_name>
```

**Process detection:**
Check if a Ghostty process is running with the session name in its command line arguments (via `ps` inspection). This is a pragmatic approach given Ghostty's lack of a control API on macOS.

**Limitations (macOS, current Ghostty):**
- Cannot programmatically add tabs to an existing Ghostty window
- Cannot programmatically focus a specific Ghostty window
- Each `open -na` invocation creates a new window (separate Ghostty instance)

**User workflow for single-window operation:**
The user manually creates Ghostty tabs (Cmd+T) and uses the CLI to launch/attach sessions within each tab. The app cannot automate the single-window layout, but the two-level navigation (Cmd+Shift+Bracket for Ghostty tabs, Cmd+Bracket for tmux windows) works naturally.

### 5.3 Future Emulator Support

The `Emulator` trait allows adding support for other terminals (Alacritty, WezTerm, Kitty, etc.) without changing the core library. Each implementation provides its own launch command and process detection logic. The user selects their preferred emulator in `settings.json`.

---

## 6. Theme and Color Control

### 6.1 Color Application

Each terminal group has a color (hex string, e.g., `#f97316`). The color is applied to the tmux session's status bar at runtime via tmux options. No layout files or config file generation required.

### 6.2 tmux Styling Commands

Applied when a session is created and whenever the color changes:

```bash
# Compute dimmed color (color / 3) for inactive elements
# These values are computed by the library, not hardcoded

tmux set-option -t <session> status-style "bg=<color>,fg=#000000"
tmux set-option -t <session> status-left "#[bg=<darker>,fg=#ffffff,bold] <group_name> #[default]"
tmux set-option -t <session> window-status-format "#[fg=#000000]  #I: #W  "
tmux set-option -t <session> window-status-current-format "#[fg=<color>,bg=#000000,bold] #I: #W #[default]"
tmux set-option -t <session> window-status-separator ""
tmux set-option -t <session> status-position top
```

### 6.3 Live Color Updates

When the user changes a group's color (via GUI or CLI):

1. Update the tmux user option: `tmux set-option -t <session> @muster_color "<new_color>"`
2. Re-apply the styling commands immediately (the status bar updates instantly)
3. Emit event to GUI for beacon/UI color update

No session restart required. No file writes for a runtime color change. If the user wants the new color to persist to the profile, that's an explicit save operation.

### 6.4 Frontend Color Usage

The GUI uses group colors for:
- File browser beacons (colored dots next to directories with active terminals)
- Group launcher UI (profile cards, tab indicators)
- Group terminal overlay (tab bar accent, active tab indicator)

All frontend color rendering reads from the same `color` field in the session metadata / profile.

---

## 7. Key Bindings

### 7.1 tmux Configuration

The library generates a minimal tmux configuration for managed sessions:

```tmux
# Mouse support (clickable tabs)
set -g mouse on

# Cmd+[ / Cmd+] mapped at the Ghostty level (see 7.2)
# These arrive as prefix + p / prefix + n

# Direct window selection via Option+number
bind-key -n M-1 select-window -t :0
bind-key -n M-2 select-window -t :1
bind-key -n M-3 select-window -t :2
bind-key -n M-4 select-window -t :3
bind-key -n M-5 select-window -t :4
bind-key -n M-6 select-window -t :5
bind-key -n M-7 select-window -t :6
bind-key -n M-8 select-window -t :7
bind-key -n M-9 select-window -t :8
```

This configuration is applied per-session or via a shared config file sourced by managed sessions.

### 7.2 Ghostty Key Bindings

The library can write or recommend Ghostty keybinding configuration:

```ghostty
# tmux tab navigation (Cmd+bracket for prev/next tmux window)
keybind = super+left_bracket=text:\x02p
keybind = super+right_bracket=text:\x02n
```

These coexist with Ghostty's native keybindings:
- **Cmd+[** / **Cmd+]** — switch tmux windows (tabs within a group)
- **Cmd+Shift+[** / **Cmd+Shift+]** — switch Ghostty tabs (groups)
- **Cmd+T** — new Ghostty tab
- **Cmd+1-9** — Ghostty tab by number (or remapped to tmux windows)
- **Mouse click** — click tmux tab in status bar to switch

### 7.3 Key Binding Philosophy

The library does not forcibly override user configuration. It provides:
1. A recommended configuration that the user can adopt
2. Programmatic helpers to write config fragments
3. Detection of existing configuration to avoid conflicts

---

## 8. Window and Tab Management

### 8.1 Terminology Mapping

| App Concept | tmux Concept | Ghostty Concept |
|-------------|-------------|-----------------|
| Terminal Group | Session | Window (one per group) |
| Tab | Window | N/A (managed by tmux) |
| Terminal | Pane | N/A (managed by tmux) |

### 8.2 Operations

**List groups:**
Query tmux for `muster_*` sessions with their `@muster_*` user options. Merge with profiles from `profiles.json` where a profile reference exists.

**Open group:**
If session exists → launch emulator attached to it. If session doesn't exist but profile exists → create session from profile, then launch emulator. Optionally switch to a specific tab by index.

**Close group:**
Kill the tmux session. The emulator window closes automatically. Metadata dies with the session — no file cleanup needed.

**Add tab to group:**
`tmux new-window -t <session> -n <name> -c <cwd>`. Control mode pushes `%window-add`. GUI updates reactively.

**Close tab:**
`tmux kill-window -t <session>:<index>`. Control mode pushes `%window-close`. GUI updates reactively. If last window, session dies, emulator closes.

**Switch tab:**
`tmux select-window -t <session>:<index>`. Used when the user clicks a beacon or selects a tab in the GUI.

**Rename tab:**
`tmux rename-window -t <session>:<index> <new_name>`. Control mode pushes `%window-renamed`.

### 8.3 Tab Index Mapping

tmux windows have a stable index (`window_index`). The library uses these indices directly — no secondary mapping, no generated IDs, no index translation. When the GUI needs to reference a tab, it uses the tmux window index. When switching tabs, it passes the tmux window index to `select-window`.

This eliminates the entire class of index-mapping bugs from the previous architecture.

---

## 9. Session State Synchronization

### 9.1 Push-Based Synchronization (Control Mode)

For each active session, a control mode connection delivers:
- Window add/close/rename events
- Session lifecycle events

The library translates these into application events that the GUI consumes:

```rust
pub enum MusterEvent {
    // Core events
    TabAdded { session: String, window_index: u32, name: String },
    TabClosed { session: String, window_index: u32 },
    TabRenamed { session: String, window_index: u32, name: String },
    ActiveTabChanged { session: String, window_index: u32 },
    SessionEnded { session: String },
    CwdChanged { session: String, window_index: u32, cwd: PathBuf },

    // Post-core events (added when features are implemented)
    // ProcessExited { session: String, window_index: u32, exit_code: Option<i32> },
    // Output { session: String, window_index: u32, data: String },
}
```

### 9.2 CWD Tracking

tmux tracks `pane_current_path` natively (it parses OSC 7 from the shell). The library queries CWDs via:

```bash
tmux list-windows -t <session> -F '#{window_index} #{pane_current_path}'
```

CWD change detection strategy (in order of preference, determined during implementation):

1. **`%subscription-changed` (preferred)** — tmux control mode supports format subscriptions via `refresh-client -B <name>:<interval>:<format>`. By subscribing to `#{pane_current_path}` for each tracked pane, the library can receive `%subscription-changed` notifications when CWDs change. This is a genuine push mechanism requiring no polling. Needs verification during implementation to confirm the subscription fires reliably on CWD changes.

2. **On-demand query** — query CWDs when the user views a group or the file browser needs beacon data. This is sufficient for beacon rendering — beacons don't need real-time CWD tracking, they need accurate CWDs when the user is looking at a directory listing.

3. **Periodic query (fallback)** — poll `list-windows` at a reasonable interval (e.g., 5s) only for CWDs, since everything else is pushed. Only if option 1 proves unreliable.

The key point: even in the worst case (option 3), the polling is limited to a single lightweight query per session for CWD data only. All other state (window lifecycle, names, active window) is push-based via control mode. This is fundamentally different from the previous architecture where polling was the *entire* state sync mechanism.

### 9.3 GUI Integration (Tauri)

The Tauri application layer:
1. Calls library initialization on app startup (reconciles state)
2. Subscribes to `MusterEvent` stream
3. On each event: queries library for current group state, emits to frontend via Tauri event
4. Frontend replaces its state reactively (same `groups-updated` pattern, but now backed by tmux truth)

### 9.4 What Is NOT Synchronized

- Terminal output (user sees it directly in the emulator)
- Scrollback buffer (owned by tmux, viewed in emulator)
- Pane splits within a window (possible future feature, not in scope)

---

## 10. Library API Surface

### 10.1 Core API

```rust
pub struct Muster {
    config_dir: PathBuf,
    tmux: TmuxClient,
    emulator: Box<dyn Emulator>,
}

impl Muster {
    /// Initialize: discover tmux, reconcile state, connect control mode.
    pub async fn init(config_dir: PathBuf, settings: Settings) -> Result<Self, Error>;

    // --- Profiles ---
    pub async fn list_profiles(&self) -> Vec<Profile>;
    pub async fn get_profile(&self, id: &str) -> Option<Profile>;
    pub async fn save_profile(&self, profile: Profile) -> Result<(), Error>;
    pub async fn delete_profile(&self, id: &str) -> Result<(), Error>;

    // --- Sessions (live groups) ---
    pub async fn list_sessions(&self) -> Vec<SessionInfo>;
    pub async fn launch(&self, profile_id: &str) -> Result<SessionInfo, Error>;
    pub async fn attach(&self, session_name: &str, window_index: Option<u32>) -> Result<(), Error>;
    pub async fn destroy(&self, session_name: &str) -> Result<(), Error>;

    // --- Tabs (tmux windows) ---
    pub async fn add_tab(&self, session: &str, name: &str, cwd: &str, command: Option<&str>) -> Result<u32, Error>;
    pub async fn close_tab(&self, session: &str, window_index: u32) -> Result<(), Error>;
    pub async fn switch_tab(&self, session: &str, window_index: u32) -> Result<(), Error>;

    // --- Appearance ---
    pub async fn set_color(&self, session: &str, color: &str) -> Result<(), Error>;
    pub async fn apply_theme(&self, session: &str) -> Result<(), Error>;

    // --- State observation ---
    pub async fn get_session_state(&self, session: &str) -> Result<SessionState, Error>;
    pub fn subscribe(&self) -> broadcast::Receiver<MusterEvent>;

    // --- Emulator ---
    pub async fn open_emulator(&self, session: &str) -> Result<(), Error>;
}
```

### 10.2 Data Types

```rust
pub struct Profile {
    pub id: String,
    pub name: String,
    pub color: String,
    pub tabs: Vec<TabProfile>,
}

pub struct TabProfile {
    pub name: String,
    pub cwd: String,
    pub command: Option<String>,
}

pub struct SessionInfo {
    pub session_name: String,
    pub profile_id: Option<String>,
    pub name: String,
    pub color: String,
    pub windows: Vec<WindowInfo>,
    pub active_window: u32,
}

pub struct WindowInfo {
    pub index: u32,
    pub name: String,
    pub cwd: PathBuf,
}

pub struct SessionState {
    pub windows: Vec<WindowInfo>,
    pub active_window: u32,
    pub attached: bool,
}
```

### 10.3 Testability

The library is testable at multiple levels:

**Unit tests (no tmux required):**
- Profile CRUD (reads/writes JSON files in a temp directory)
- Color computation (hex parsing, dimming, tmux style string generation)
- Session name convention (encoding/decoding profile IDs)
- Control mode stream parser (given raw control mode output, verify parsed events)

**Integration tests (tmux required, no emulator):**
- Session lifecycle: create from profile, verify windows exist, destroy
- Tab operations: add, close, rename, verify via tmux queries
- Theme application: set color, verify tmux options
- Control mode: connect, receive events on window add/close
- Startup discovery: create sessions externally with `@muster_*` options, verify library discovers them

**Integration tests mock the `Emulator` trait** — they test the library's tmux interaction without launching any GUI windows.

---

## 11. CLI Interface

### 11.1 Commands

```
muster list                              # List profiles and running sessions
muster launch <profile-name-or-id>       # Launch or attach to a profile's session
muster attach <session-name>             # Attach to a running session
muster new <name> [--cwd <dir>] [--color <hex>]  # Create ad-hoc group
muster kill <session-name>               # Destroy a session
muster add-tab <session> [--cwd <dir>] [--name <name>] [--command <cmd>]
muster profile save <name> [--from-session <name>]  # Save current session as profile
muster profile list
muster profile delete <name-or-id>
muster color <session> <hex-color>       # Change session color live
muster status                            # Show all sessions with window counts, CWDs
```

### 11.2 Behavior

- `launch` is the primary command. If the session is already running, it attaches. If not, it creates from the profile.
- When run inside a Ghostty tab, `launch` and `attach` replace the current shell with `exec tmux attach -t <session>`.
- When run outside a terminal context (e.g., from the GUI), it launches the emulator.
- `list` shows profiles with a marker for which ones have active sessions.

### 11.3 Output Format

Human-readable by default. `--json` flag for machine-readable output (used by the Tauri app if needed, though the app would typically use the library directly).

---

## 12. Integration with the Tauri Application

### 12.1 Dependency

The Tauri app depends on the `muster` library crate. It does NOT shell out to the CLI. The CLI and the Tauri app are independent consumers of the same library.

### 12.2 Tauri Commands

Tauri command handlers become thin wrappers around library calls:

```rust
#[tauri::command]
async fn group_list(tg: State<'_, Arc<Muster>>) -> Result<Vec<SessionInfo>, String> {
    tg.list_sessions().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn group_launch(tg: State<'_, Arc<Muster>>, profile_id: String) -> Result<SessionInfo, String> {
    tg.launch(&profile_id).await.map_err(|e| e.to_string())
}
```

### 12.3 Event Bridge

The Tauri app subscribes to the library's event stream and forwards to the frontend:

```rust
// In app setup:
let mut rx = muster.subscribe();
let handle = app.handle().clone();
tauri::async_runtime::spawn(async move {
    while let Ok(event) = rx.recv().await {
        // Query current state and emit to frontend
        let sessions = muster.list_sessions().await.unwrap_or_default();
        handle.emit("groups-updated", &sessions).ok();
    }
});
```

### 12.4 File Browser Beacons

The file browser queries the library for active sessions and their tab CWDs:

```rust
#[tauri::command]
async fn get_active_cwds(tg: State<'_, Arc<Muster>>) -> Result<Vec<BeaconInfo>, String> {
    let sessions = tg.list_sessions().await.map_err(|e| e.to_string())?;
    let beacons: Vec<BeaconInfo> = sessions.iter()
        .flat_map(|s| s.windows.iter().map(move |w| BeaconInfo {
            path: w.cwd.clone(),
            group_name: s.name.clone(),
            color: s.color.clone(),
        }))
        .collect();
    Ok(beacons)
}
```

Beacons are derived data, not stored state.

### 12.5 Search Integration

ParavaneFS provides filesystem search. Terminal groups provide filesystem context. The integration point is that both operate on paths — a search result can indicate "this file is in a directory with an active terminal group," and clicking a beacon from search results can jump to the relevant group.

---

## 13. Design Considerations

### 13.1 No Timing Hacks

The previous architecture relied on:
- 800ms delays for session readiness
- 500ms startup delays
- 2s polling intervals for state sync
- PID-based process tracking heuristics

This design eliminates all of these:
- **Session readiness**: tmux sessions are ready immediately after `new-session` returns. No delay needed before sending commands.
- **Startup reconciliation**: single synchronous query of tmux + state file comparison. No polling.
- **State sync**: control mode push events, not polling (except possibly for CWDs).
- **Process tracking**: query tmux for attached clients or check OS process state directly. No stale PID maps.

### 13.2 Failure Modes

**General:**
- **tmux not installed**: library returns clear error at init. App can show setup instructions.
- **tmux server not running**: `new-session` starts it automatically. No special handling needed.
- **Emulator not installed**: library returns error on `open_emulator`. Sessions still manageable via CLI.
- **Config directory permissions**: standard filesystem error handling.
- **Session killed externally**: control mode reports `%sessions-changed`. Library re-queries tmux. GUI updates. No file cleanup needed — metadata dies with the session.

**Control mode specific:**

Each control mode connection is a persistent child process with stdin/stdout pipes. This introduces specific failure modes:

- **N sessions = N persistent subprocesses**: Each managed session has one control mode client. These are lightweight (no terminal rendering, output suppressed), but resource usage scales linearly. Acceptable for the expected scale (tens of sessions, not thousands).

- **App crash leaves orphaned tmux clients**: If the library's process dies without cleanup, control mode clients remain attached to their sessions. The sessions themselves are unaffected (tmux sessions are independent of clients), but `tmux list-clients` will show stale entries. **Mitigation**: On startup reconciliation, the library queries `tmux list-clients` and detaches any stale control mode clients from a previous run (identifiable by PID no longer running, or by a client name convention).

- **Control mode connection drops**: Detected immediately — the pipe closes, the reader task gets EOF. **Mitigation**: Reconnect and issue a single `list-windows` query to catch any events missed during the gap. Since tmux is the source of truth, no state is lost — we just need to re-sync.

- **Mixed stream parsing**: Control mode interleaves notifications with command response blocks (`%begin`/`%end`). **Mitigation**: The protocol is well-defined — response blocks are framed with matching command numbers, notifications never appear inside blocks. A straightforward state machine parser handles this reliably.

### 13.3 Extensibility

- **New emulators**: implement the `Emulator` trait. Core logic unchanged.
- **Remote sessions**: tmux natively supports remote attach. The library could extend to SSH tunneled sessions.
- **Session sharing**: tmux supports multiple clients on one session. Could enable collaborative terminal viewing.

### 13.4 Competitive Landscape

The tmux ecosystem has many tools but they fall into two narrow categories, neither of which covers what this library does:

**Session switchers** (sesh, tuxmux, tmux-sessionx, sessionizer) — Fuzzy-find and jump between existing sessions. They don't create sessions from profiles, don't manage colors/themes, don't provide event subscriptions, and aren't libraries. They're interactive CLI tools.

**Profile launchers** (tmuxinator, tmuxp, tmuxrs) — Create sessions from YAML/JSON config files. Fire-and-forget: launch a session from a template, then done. No runtime management, no color theming, no state observation, no control mode.

**Libraries:**
- `libtmux` (Python, 1.1K stars) — typed Python API over tmux. Query and command focused. No control mode, no push events.
- `tmux_interface` (Rust, 64 stars) — low-level command builders for all 90 tmux commands. Self-described as experimental. Control mode unimplemented.

**What no existing tool provides:**
- Control mode event streaming (the push-based state sync that eliminates polling)
- Runtime per-session theming (applying colors to live tmux status bars)
- Session group metadata management (colors, display names, profile associations)
- A library-first Rust API for ongoing session lifecycle management
- Dual-mode (GUI + CLI) consumption of the same session management layer

This library fills a genuine gap. The Rust tmux bindings (command execution + control mode parsing) are independently useful and could attract users who just need programmatic tmux access from Rust.

### 13.5 What This Design Does NOT Do

- **Render terminal output**: the emulator does this. The app is not a terminal emulator.
- **Replace tmux**: the app is an organizational layer on top of tmux, not a reimplementation.
- **Manage non-terminal processes**: this is specifically for interactive terminal sessions, not for process supervision.
- **Session dependencies / orchestration**: starting group A before group B is process orchestration, not terminal management.
- **Auto-cleanup of idle sessions**: too opinionated and too easy to destroy in-progress work.
- **Remote / SSH sessions**: natural extension point but not in scope for the initial build.

---

## 14. Feature Roadmap

Features are organized into core (required for a working library) and post-core (built on top of the core). The core must be complete and stable before post-core features are added. The architecture accounts for all features below so that post-core additions don't require restructuring.

### 14.1 Core

The minimum viable library. Everything needed for session group lifecycle management:

- tmux command bindings (~20 commands)
- Control mode connection management and event stream parsing
- Profile CRUD (create, read, update, delete saved group templates)
- Session lifecycle (create from profile, destroy, attach)
- Window/tab management (add, close, switch, rename)
- Runtime theming (per-session color application to tmux status bar)
- Session metadata via tmux user options (`@muster_name`, `@muster_color`, `@muster_profile`)
- State reconciliation on startup
- Event subscription (`broadcast::Receiver<MusterEvent>`)
- Emulator trait + Ghostty implementation
- CLI binary with core commands (list, launch, attach, kill, new, color, status)

### 14.2 Post-Core Features

Each feature is self-contained and builds on the core without modifying it. Ordered roughly by value and implementation simplicity.

**Session snapshotting** — Capture a running session's current windows, CWDs, and layout into a profile. Implementation: query `list-windows` with format string to get window names, CWDs, and pane count, then serialize to a `Profile`. CLI: `muster profile save --from-session <name>`. Library: `save_profile_from_session(session: &str) -> Result<Profile, Error>`.

**Process status tracking** — Expose whether the process in each pane is alive or dead. tmux tracks this natively (`pane_dead`, `pane_pid`). Implementation: include `is_alive: bool` in `WindowInfo`, populated from `list-windows` format strings. The `pane-exited` hook / control mode notification can push updates. Useful for both CLI (`muster status` shows dead processes) and GUI (beacon changes to indicate a crashed dev server).

**Output capture** — Per-pane output streaming for routing terminal output to an application log console. Control mode supports this natively: `refresh-client -A '%<pane-id>:on'` enables `%output` notifications for a specific pane while all others remain suppressed. Implementation: `subscribe_output(session: &str, window_index: u32) -> broadcast::Receiver<OutputEvent>` enables output for that pane, `unsubscribe_output` re-suppresses it. This is opt-in per pane, not global — the default remains suppressed.

**Session adoption** — Bring existing (non-managed) tmux sessions under management. Implementation: rename session to add `muster_` prefix, set `@muster_*` user options. No file writes needed. CLI: `muster adopt <session-name> --color '#f97316' --name 'My Project'`. Library: `adopt(session_name: &str, name: &str, color: &str) -> Result<SessionInfo, Error>`.

**Pane layouts** — Extend profiles to define split panes within a tab. tmux supports `split-window` and named layouts (`main-vertical`, `tiled`, etc.). Implementation: add optional `panes: Vec<PaneProfile>` to `TabProfile`, apply via `split-window` + `select-layout` during session creation. This is the feature tmuxinator/tmuxp users expect.

**Environment variables** — Per-session environment variables defined in profiles. tmux supports `set-environment -t <session> VAR value`. Implementation: add optional `env: HashMap<String, String>` to `Profile`, apply via `set-environment` during session creation. Useful for `NODE_ENV`, project-specific paths, etc.

**Per-session tmux config** — Manage tmux settings (mouse, key bindings, status bar position) per session without touching the user's global `~/.tmux.conf`. All via `set-option -t <session>`. Implementation: add optional `tmux_options: HashMap<String, String>` to profile or settings, apply during session creation.

**Shell integration** — A shell hook that detects when the user `cd`s into a directory associated with a profile and suggests launching. Implementation: `muster shell-init <shell>` outputs a shell function that queries profiles on directory change. Supports fish, bash, zsh.

### 14.3 Architectural Considerations for Post-Core

The core architecture naturally supports these features without restructuring because:

- **Output capture** uses the same control mode connection (just toggling `refresh-client -A` per pane)
- **Process status** is additional data in existing `list-windows` queries (adding format fields)
- **Session snapshotting** is a read-only operation on existing tmux queries → profile serialization
- **Pane layouts** extend the session creation flow (additional `split-window` commands after `new-window`)
- **Environment variables and tmux config** are additional `set-environment` / `set-option` commands in the session creation flow
- **Session adoption** is metadata operations (`rename-session` + `set-option @muster_*`)
- **Shell integration** is a CLI-only feature that queries profiles — no library changes needed
