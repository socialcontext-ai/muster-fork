# Introduction

Muster is a Rust library and CLI for terminal session group management built on tmux. It organizes terminal sessions into named, color-coded groups with saved profiles, runtime theming, and push-based state synchronization via tmux control mode.

## What Muster Does

- **Organizes terminals by project** — group related tabs (shell, server, logs) into a single named session
- **Saves profiles** — define reusable templates for your project setups
- **Applies color themes** — each group gets a distinct color in the tmux status bar
- **Syncs state via tmux** — no polling, no stale state files; tmux is the single source of truth
- **Provides a library API** — the CLI is a thin consumer of the `muster` Rust library; the API is designed for GUI integration

## Who It's For

Developers who work across multiple projects and maintain numerous terminal sessions — development servers, test runners, build watchers — spread across many terminal tabs with no organizational structure. Muster turns that chaos into named, color-coded groups you can launch, switch between, and tear down with single commands.

## How It Works

Muster is a tmux interface layer, not a tmux replacement. It creates and manages tmux sessions on your behalf, storing metadata (name, color, profile reference) as tmux user options on the sessions themselves. Profiles are saved templates; running state lives entirely in tmux.

The architecture is a Cargo workspace with three crates:

| Crate | Purpose |
|-------|---------|
| `muster` | Library — tmux bindings, profiles, theming, control mode |
| `muster-cli` | CLI binary |
| `muster-notify` | macOS notification helper (optional) |

## Documentation Structure

- **[Getting Started](getting-started/installation.md)** — installation and first steps
- **[User Guide](user-guide/concepts.md)** — concepts, profiles, sessions, and features
- **[CLI Reference](cli-reference.md)** — complete command documentation (auto-generated)
- **[Architecture](architecture/overview.md)** — internal design and tmux interface details
- **[Development](development/testing.md)** — testing and contributing
- **[API Reference](api/)** — rustdoc for the library crate
