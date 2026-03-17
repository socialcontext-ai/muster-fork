# Concepts

## Terminal Group

A named tmux session containing one or more windows (tabs), each with a working directory and optional startup command. Groups have a display name, color, and optional profile reference.

| App Concept | tmux Concept |
|-------------|-------------|
| Terminal Group | Session |
| Tab | Window |
| Terminal | Pane |

## Profile

A saved template for creating a group. Stored in `~/.config/muster/profiles.json`. Contains the group's name, color, and tab definitions.

Profiles are *not* running state — they're blueprints. You can have a profile without a running session, or a running session created ad-hoc without a profile.

## Session

A running tmux session managed by muster. Session names are prefixed with `muster_` to distinguish them from personal tmux sessions. Application metadata (name, color, profile ID) is stored as tmux user options (`@muster_name`, `@muster_color`, `@muster_profile`) on the session itself — no separate state file.

## Sources of Truth

There are exactly two sources of truth:

| Source | Owns |
|--------|------|
| **tmux** | All running state: windows, CWDs, active window, plus `@muster_*` metadata |
| **Config directory** | Saved profiles and settings — never runtime state |

There is no application-level cache. When you need session state, muster asks tmux. This eliminates state synchronization bugs entirely.

## Session Naming

All managed sessions use the prefix `muster_` followed by a slugified profile ID:

```
muster_myproject
muster_web-app
```

This lets muster distinguish its sessions from your personal tmux sessions.
