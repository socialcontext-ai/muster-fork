# muster-cli Refactor Plan

Three phases: module extraction, integration tests, error handling polish.
Phase 1 first — the other two build on it.

---

## Phase 1: Module Extraction [DONE]

### Structure

```
crates/muster-cli/src/
  main.rs              -- main(), run(), top-level match dispatch (~100 lines)
  cli.rs               -- unchanged (clap definitions, library target)
  commands/
    mod.rs             -- CommandContext, session filter helper, re-exports
    list.rs            -- List
    launch.rs          -- Launch
    attach.rs          -- Attach
    kill.rs            -- Kill
    new.rs             -- New
    color.rs           -- Color
    inspect.rs         -- Ps, Ports, Top (share session filtering pattern)
    status.rs          -- Status
    peek.rs            -- Peek
    pin.rs             -- Pin, Unpin
    hooks.rs           -- SyncRename, PaneDied, Bell
    profile.rs         -- Profile { action } (all ProfileAction variants)
    notifications.rs   -- Notifications { action }
  format.rs            -- color_dot, format_memory, terminal formatting
  proctree.rs          -- ProcessInfo, ProcessTree, parsing/rendering
  ports.rs             -- ListeningPort, MatchedPort, parsing
  resources.rs         -- ResourceEntry, GpuProcessInfo, parsing/rendering
  editing.rs           -- EditableProfile/Tab/Pane, conversions
  tabs.rs              -- parse_tab, build_tabs
  terminal.rs          -- exec_tmux_attach, tmux_path, notification helpers
```

### CommandContext

Shared state passed to every command handler:

```rust
pub(crate) struct CommandContext {
    pub muster: Muster,
    pub settings: Settings,
    pub config_dir: PathBuf,
    pub json: bool,
}
```

Each command module exposes `pub(crate) fn execute(ctx: &CommandContext, ...) -> Result<()>`
where `...` is command-specific args destructured from the `Command` enum.

### Utility Modules

| Module         | Contents |
|----------------|----------|
| `tabs.rs`      | `parse_tab`, `build_tabs` |
| `format.rs`    | `color_dot`, `format_memory` |
| `editing.rs`   | `EditableProfile/Tab/Pane`, conversions |
| `terminal.rs`  | `tmux_path`, `exec_tmux_attach`, notification functions |
| `proctree.rs`  | Process tree structs and functions |
| `ports.rs`     | Listening port structs and functions |
| `resources.rs` | Resource/GPU structs and functions |

### Command Modules

| Module              | Command variants |
|---------------------|-----------------|
| `commands/list.rs`  | List |
| `commands/launch.rs`| Launch |
| `commands/attach.rs`| Attach |
| `commands/kill.rs`  | Kill |
| `commands/new.rs`   | New |
| `commands/color.rs` | Color |
| `commands/inspect.rs`| Ps, Ports, Top |
| `commands/status.rs`| Status |
| `commands/peek.rs`  | Peek |
| `commands/pin.rs`   | Pin, Unpin |
| `commands/hooks.rs` | SyncRename, PaneDied, Bell |
| `commands/profile.rs`| All ProfileAction variants |
| `commands/notifications.rs` | Notifications Setup/Remove/Test |

### Results

- `main.rs`: 2141 → 125 lines (dispatch only)
- `CommandContext` carries shared state to all handlers
- `filter_sessions` helper in `commands/mod.rs` deduplicates Ps/Ports/Top logic
- Tests migrated to their respective modules (21 passing)
- Zero clippy warnings, `cargo fmt` clean

**Verified**: `cargo clippy --workspace` zero warnings. `cargo fmt --check` passes.
81 tests passed, 26 ignored (tmux-dependent).

---

## Phase 2: Integration Test Improvements

### Current State
- All 21 tests are unit tests for parser functions in main.rs
- No integration tests exist (`crates/muster-cli/tests/` doesn't exist)
- 26 tests skipped in earlier runs were tmux-dependent tests in the core library

### Approach

1. **Profile CRUD tests** (no tmux needed) — `assert_cmd` tests using temp config dir
2. **`muster list --json`** — with temp config dir, no tmux required
3. **Parser snapshot tests** — `insta` snapshots for `render_tree` and display functions
4. **Tmux-dependent tests** — behind `#[ignore]`, use `tmux -L muster_test` isolated server

---

## Phase 3: Error Handling Polish

### Soft Error Expansion

Additional tmux stderr messages to treat as non-errors for query commands:
- `"no sessions"` / `"session not found"`
- `"window not found"` / `"pane not found"`
- `"server exited"` / `"server not found"`

### Systematic Approach

1. Add `cmd_soft()` method to `TmuxClient` — caller provides context-specific soft patterns
2. Audit all `cmd()` call sites for appropriate soft error handling
3. Replace `process::exit(1)` in command handlers with proper `Err(...)` returns
4. Unify error display through `run()` → `main()`
