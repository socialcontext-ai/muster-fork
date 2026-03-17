# Contributing

## Development Commands

```bash
cargo t              # alias for cargo nextest run
cargo clippy         # lint
cargo fmt --check    # format check
cargo doc --no-deps  # build docs (check for warnings)
```

## Project Structure

```
crates/
├── muster/             # Library crate (tmux bindings, profiles, theming, control mode)
├── muster-cli/         # CLI binary crate
│   └── src/
│       ├── main.rs         # Entry point, CLI dispatch (~125 lines)
│       ├── cli.rs          # Clap command definitions (library target)
│       ├── commands/       # One module per command (list, up/launch, down/kill, etc.)
│       ├── format.rs       # Terminal formatting (color dots, memory display)
│       ├── tabs.rs         # Tab definition parsing
│       ├── editing.rs      # TOML profile editing types
│       ├── terminal.rs     # tmux attach, notification helpers
│       ├── proctree.rs     # Process tree building/rendering
│       ├── ports.rs        # Listening port detection
│       └── resources.rs    # CPU/memory/GPU resource collection
└── muster-notify/      # macOS notification helper
docs/                   # mdBook documentation (this site)
```

## Documentation

### Building Docs Locally

```bash
# mdBook user guide
mdbook serve docs

# API reference (rustdoc)
cargo doc --no-deps --open

# Regenerate CLI reference
cargo run --example gen_cli_docs -p muster-cli > docs/src/cli-reference.md
```

### Writing Doc Comments

All public types, functions, and modules should have rustdoc comments:

- `//!` for module-level docs
- `///` for public types, functions, and methods

## Code Quality

The workspace enforces:

- `unsafe_code = "deny"` — no unsafe Rust
- `clippy::all` and `clippy::pedantic` — comprehensive linting
- All public items documented
