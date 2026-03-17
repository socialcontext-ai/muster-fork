# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.1] - 2026-03-17

Internal refactoring, error handling, and test infrastructure. No user-facing changes.

### Changed
- Refactored CLI from a 2141-line monolith into focused modules: `commands/`
  directory with one module per command, plus utility modules for formatting,
  process trees, ports, resources, tab parsing, editing, and terminal operations.
  `main.rs` is now 125 lines of pure dispatch.
- Unified CLI error handling: all command handlers return `CliError` instead
  of calling `process::exit(1)`. Reduced exit calls from 23 to 3.
- Updated contributing guide with CLI module structure.

### Added
- `CliError` type with `User`/`Internal` variants and `bail!` macro.
- 28 CLI integration tests via `assert_cmd`: profile CRUD, list, color,
  error cases, and no-session behavior for status/ps/ports/top.
- Tests use `TMUX_TMPDIR` isolation and seeded temp config dirs.
- Coverage baseline: 60% line coverage (135 tests with `--run-ignored all`).
- Refactor plan document (`docs/refactor-plan.md`).
- This changelog.

### Fixed
- Added `"server exited"` and `"server not found"` to tmux soft error patterns,
  preventing crashes when the tmux server shuts down between operations.

## [0.1.0] - 2026-03-17

Initial tagged release. Baseline for all prior development.

### Core
- tmux client: command execution, output parsing, session/window/pane CRUD
- Control mode: event stream parsing and push-based subscription
- Profile management with atomic JSON persistence
- Session lifecycle: create from profile, destroy, resolve by name/ID
- Runtime theming: per-session color application with dimmed variants
- Named color system with Tailwind shade variants and CSS named colors
- Settings management (terminal preference, shell)

### CLI Commands
- `list` — profiles and running sessions
- `launch` / `attach` / `kill` — session lifecycle
- `new` — ad-hoc session creation with inline profile
- `color` — live color changes, `--list` for available colors
- `ps` — process trees inside sessions
- `ports` — listening TCP ports matched to sessions
- `top` — CPU, memory, GPU resource usage per session/window
- `status` — detailed session and window state
- `peek` — capture recent terminal output from windows
- `pin` / `unpin` — save/remove window layouts to profiles
- `profile` — full CRUD (list, show, save, edit, update, delete, add-tab, remove-tab)
- `notifications` — macOS notification helper setup/remove/test

### Infrastructure
- Workspace: `muster` (library), `muster-cli` (binary), `muster-notify` (macOS helper)
- Edition 2024, MSRV 1.85
- MIT OR Apache-2.0 dual license
- Structured tracing via `tracing` crate
- Snapshot testing with `insta`
- mdBook documentation site
- Rustdoc comments on all public items

### Bug Fixes
- Handle missing tmux socket (`error connecting to`) as soft error
- Fix nested tmux: open new terminal window instead of nesting
- Fix notification delivery: spawn async and force new instances
- Batch launch commands via `tmux source-file` for reliability
- Fix parallel integration test stability
- Strip `CLAUDECODE` env var from tmux sessions
