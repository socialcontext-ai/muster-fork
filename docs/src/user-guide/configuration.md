# Configuration

## Config Directory

Muster stores configuration in `~/.config/muster/` by default. Override with `--config-dir` or the `MUSTER_CONFIG_DIR` environment variable.

```
~/.config/muster/
├── profiles.json             # Saved terminal group profiles
├── settings.json             # Global settings
├── logs/                     # Death snapshots
│   └── <session_name>/
│       └── <window_name>.last
└── Muster.app/               # macOS notification helper (optional)
```

## Settings (`settings.json`)

```json
{
  "terminal": "ghostty",
  "shell": "/usr/local/bin/fish",
  "tmux_path": null
}
```

Settings can be viewed and updated with the `muster settings` command:

```bash
# Show current settings
muster settings

# Update a setting
muster settings --terminal ghostty
muster settings --shell /usr/local/bin/fish
muster settings --tmux-path /usr/local/bin/tmux
```

### `terminal`

The terminal emulator to open when launching a session from inside tmux. If omitted, muster uses the platform default (Terminal.app on macOS; detected from PATH on Linux).

Supported values: `ghostty`, `kitty`, `alacritty`, `wezterm`, `terminal` (Terminal.app), `iterm2`.

### `shell`

Overrides the default shell for new tmux panes. If omitted, muster uses `$SHELL`. Set this if your `$SHELL` differs from the shell you actually use (common on macOS where `$SHELL` defaults to `/bin/zsh`).

### `tmux_path`

Overrides tmux discovery from `$PATH`. Set this if tmux is installed in a non-standard location.

## Tmux Options Set by Muster

Muster applies a small set of tmux options to each managed session. These are
required for core functionality and should not be overridden in your
`~/.tmux.conf` for muster sessions. Everything else is left to your tmux
configuration.

### Session-level options

| Option | Value | Why |
|--------|-------|-----|
| `status-style` | `bg=<color>,fg=<fg>` | Color-coded status bar per session. This is how muster visually distinguishes sessions. |
| `status-left` | Session name label | Displays the session's display name in the status bar. |

### Per-window options

| Option | Value | Why |
|--------|-------|-----|
| `window-status-format` | Colored tab format | Styles tabs to match the session color. Unpinned tabs show a red dot (●). |
| `window-status-current-format` | Active tab format | Highlights the active tab with a contrasting background. |
| `window-status-separator` | Empty string | Removes the default separator between tabs for a cleaner look. |
| `remain-on-exit` | `on` | Keeps dead panes alive so muster can capture output and send notifications via the `pane-died` hook. |

### Session-level hooks

| Hook | Purpose |
|------|---------|
| `after-new-window` | Applies neutral (unpinned) styling to new tabs. |
| `after-split-window` | Marks pinned windows as layout-stale when panes are split. |
| `after-rename-window` | Syncs tab renames back to the saved profile. |
| `alert-bell` | Sends a notification when a bell fires in a pane. |

### Per-window hooks

| Hook | Purpose |
|------|---------|
| `pane-died` | Captures output from dead panes, sends a notification, and cleans up ephemeral tabs. |

### Options muster does NOT set

Muster intentionally leaves these to your `~/.tmux.conf`:

- **`mouse`** — Enable with `set -g mouse on` if you want clickable tabs and
  mouse scrolling. Be aware that tmux's mouse mode changes selection behavior
  (selections clear on mouse-up and auto-copy to the tmux buffer) and may cause
  right-click context menus to dismiss immediately — these are tmux limitations,
  not muster bugs.
- **`status-position`** — Defaults to `bottom` in tmux. Add
  `set -g status-position top` to your `~/.tmux.conf` if you prefer the tab bar
  at the top.
- **`mode-keys`** — Set `set -g mode-keys vi` if you want vi-style copy mode.

Any of these can also be set per-profile using the `tmux_options` field:

```json
{
  "tmux_options": {
    "mouse": "on",
    "status-position": "top"
  }
}
```

## Profiles (`profiles.json`)

See [Profiles](profiles.md) for the full profile schema and management commands.

## Shell Integration

Muster can suggest launching profiles when you `cd` into a directory associated with one. Add the shell hook to your shell config:

**Fish** — add to `~/.config/fish/config.fish`:

```fish
muster shell-init fish | source
```

**Bash** — add to `~/.bashrc` or `~/.bash_profile`:

```bash
eval "$(muster shell-init bash)"
```

**Zsh** — add to `~/.zshrc`:

```zsh
eval "$(muster shell-init zsh)"
```

After setup, when you `cd` into a directory that matches a profile tab's CWD, muster prints a suggestion:

```
muster: profile 'webapp' matches this directory. Run: muster up webapp
```
