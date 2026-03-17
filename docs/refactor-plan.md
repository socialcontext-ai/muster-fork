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

## Phase 2: Integration Test Improvements [DONE]

### CLI Integration Tests

28 `assert_cmd` integration tests in `crates/muster-cli/tests/cli_integration.rs`:
- Profile CRUD: list, show, save, delete, add-tab, remove-tab, update (text + JSON)
- List command: profiles, empty config, JSON output
- Color: `--list` in text and JSON
- Error cases: not found, validation failures
- No-session behavior: status, ps, ports, top

Tests use `TMUX_TMPDIR` to isolate from the real tmux server and
`tempfile` config dirs with seeded `profiles.json`.

### Tmux-Dependent Tests

26 existing tests in the `muster` library crate cover tmux client CRUD,
session lifecycle, theme application, and control mode. These are marked
`#[ignore]` and run via `cargo nextest run --run-ignored all`. The nextest
config serializes them via the `tmux-integration` test group.

### Coverage Baseline (with `--run-ignored all`)

- **Overall**: 60.0% line coverage (135 tests)
- **config/profile.rs**: 97.3% | **config/settings.rs**: 98.6%
- **session/mod.rs**: 87.5% | **session/theme.rs**: 91.6%
- **tmux/client.rs**: 87.1% | **tmux/control.rs**: 79.8%
- **muster.rs**: 28.6% (facade — many methods need live sessions)
- **muster-notify**: 0% (macOS ObjC notification helper, untestable in harness)

Coverage tooling: `cargo llvm-cov nextest --run-ignored all`

---

## Phase 3: Error Handling Polish [DONE]

### CLI Error Type

Added `crate::error::CliError` with two variants:
- `User(String)` — user-facing messages displayed as-is (e.g. "Profile not found: foo")
- `Internal(Box<dyn Error>)` — propagated library/infra errors

All command handlers return `crate::error::Result`. The `bail!` macro provides
ergonomic early returns for user-facing errors.

### process::exit Elimination

Reduced `process::exit` calls from 23 to 3:
- `terminal.rs` (2): `exec_tmux_attach` is `-> !`, exits are inherent to exec/fork
- `main.rs` (1): top-level error handler

All other error paths now return `Err(CliError)` through the normal Result chain.

### Soft Error Expansion

Added `"server exited"` and `"server not found"` to `TmuxClient::cmd()` soft
error patterns, joining existing `"no server running"`, `"no current session"`,
and `"error connecting to"`.

Audited all `cmd()` call sites — the soft patterns are server-availability
checks that apply universally. A `cmd_soft()` variant was not needed since
no call site requires context-specific soft error handling.

**Verified**: `cargo clippy --workspace` zero warnings. `cargo fmt --check` passes.
81 tests passed, 26 ignored (tmux-dependent).
