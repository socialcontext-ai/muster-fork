Test again

# Muster

Terminal session group management built on tmux. A Rust library and CLI for organizing terminal sessions into named, color-coded groups with saved profiles, runtime theming, and push-based state synchronization via tmux control mode.

## Prerequisites

- **tmux** — installed and available in PATH
- **Rust** — 2024 edition (for building from source)

## Installation

```bash
cargo install muster-cli
```

Or from source:

```bash
cargo install --path crates/muster-cli
```

## Quick Start

```bash
muster profile save myproject --tab 'Shell:~/work/myproject' --color '#f97316'
muster up myproject
# You're inside a tmux session. Detach with Ctrl-b d.
muster status           # check what's running
muster up myproject     # reattach
muster down myproject   # tear down
```

## Documentation

Full documentation is available at **[scott2b.github.io/muster/](https://scott2b.github.io/muster/)**:

- [User Guide](https://scott2b.github.io/muster/) — concepts, profiles, sessions, and features
- [CLI Reference](https://scott2b.github.io/muster/cli-reference.html) — complete command docs
- [Architecture](https://scott2b.github.io/muster/architecture/overview.html) — design and tmux interface
- [API Reference](https://scott2b.github.io/muster/api/muster/) — rustdoc for the library crate

### Building Docs Locally

```bash
mdbook serve docs                    # user guide
cargo doc --no-deps --open           # API reference
```

## Development

```bash
cargo nextest run                    # unit tests
cargo nextest run --run-ignored all  # integration tests (requires tmux)
cargo clippy                         # lint
cargo fmt --check                    # format check
```
