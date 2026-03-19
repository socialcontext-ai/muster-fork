# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.5.4] - 2026-03-19

### Fixed
- `muster ports` no longer shows duplicate entries when a process listens on both
  IPv4 and IPv6 for the same port. Deduplicates by (port, pid) ([#4]).
- `muster profile edit` now splits `$EDITOR` on whitespace so values like
  `"code --wait"` work correctly instead of treating the entire string as a
  binary path ([#1]).
- `muster profile edit` error messages now include the specific operation that
  failed, the current `$EDITOR` value, and the path to `profiles.json` as a
  direct-edit workaround.

### Changed
- `muster ports` output now includes PID column and a header row for readability.

[#1]: https://github.com/scott2b/muster/issues/1
[#4]: https://github.com/scott2b/muster/issues/4

---

## [0.5.3] - 2026-03-19

### Fixed
- `muster profile edit` now provides contextual error messages for all failure
  points: temp file creation, writing, editor launch, and reading back. Previously
  all of these surfaced as a bare OS error with no indication of which step failed
  ([#1]).

[#1-2]: https://github.com/scott2b/muster/issues/1

---

## [0.5.2] - 2026-03-18

### Fixed
- `muster profile edit` now reports which editor binary failed instead of a bare
  "No such file or directory" OS error ([#1]).
- `muster profile save` updates an existing profile instead of erroring with
  "duplicate profile" when the name already exists, supporting the natural workflow
  of `muster new foo` → `muster profile save foo --from-session foo` ([#2]).

[#1]: https://github.com/scott2b/muster/issues/1
[#2]: https://github.com/scott2b/muster/issues/2

---

## [0.5.1] - 2026-03-18

### Fixed
- Add version specifier to internal `muster` workspace dependency (required for crates.io publishing).

---

## [0.5.0] - 2026-03-17

### Added
- `muster adopt <session>` — bring a plain tmux session under muster management.
  Renames to `muster_<id>`, applies muster theming, and attaches. `--name` sets
  display name; `--color` sets session color; `--save` snapshots current windows
  into a profile and pins them immediately (red dots cleared on adopt); `--detach`
  skips attach.
- `muster release <session>` — remove muster management from a session while
  keeping it alive. Strips all theming, hooks, and metadata; renames back to plain
  tmux name. Profile is preserved for future `muster up`.
- `muster shell-init <shell>` — print shell integration code for fish/bash/zsh.
  Hooks `cd` to suggest `muster up` when entering a directory matching a saved
  profile. Eval in shell rc: `muster shell-init fish | source`.
- `env` field on profiles — key/value environment variables applied to the tmux
  session via `set-environment` at launch.
- `tmux_options` field on profiles — key/value tmux options applied to the session
  via `set-option` at launch (e.g. `mouse`, `history-limit`).
- `muster profile save --from-session <session>` — snapshot current windows of a
  live session into a new profile. Pins all windows immediately so red dots clear.
- `count_unpinned_windows()` — library function counting windows without muster pin.
- `muster list` now shows unpinned window count in red when a session has ephemeral
  (unpinned) tabs, e.g. `(3 windows, 1 unpinned)`.
- Workflows documentation page covering six concrete session/profile lifecycle
  scenarios: Profile First, Session First, Adopting Plain tmux, Formalizing
  Ephemeral, Editing + Bouncing, and Releasing.

### Changed
- `muster up` falls back to `resolve_session` when no matching profile is found,
  so `muster up <session-name>` works for adopted sessions.
- `Profile` now derives `Default`; all internal struct literals use
  `..Profile::default()` instead of explicit empty `HashMap::new()` calls.

---

## [0.4.0] - 2026-03-17

### Changed
- `muster attach` is now hidden; use `muster up --tab <index>` to attach and
  switch to a specific tab. `attach` still works for backwards compatibility.
- Consistent "tab" terminology throughout CLI help text and documentation.
  (`--window` on `attach` is now `--tab`; `peek` positional args renamed from
  `WINDOWS` to `TABS`.)

---

## [0.3.0] - 2026-03-17

### Changed
- `muster launch` renamed to `muster up`; `muster kill` renamed to `muster down`.
  `launch` and `kill` remain as aliases for backwards compatibility.

---

## [0.2.0] - 2026-03-17

### Added
- `muster settings` command — show and update terminal/shell/tmux settings
  without hand-editing JSON. Flags: `--terminal`, `--shell`, `--tmux-path`.
  Displays `(default)` when terminal is not explicitly configured.
- Linux terminal support: `detect_terminal()` probes PATH for ghostty, kitty,
  alacritty, wezterm, gnome-terminal, konsole, xfce4-terminal, x-terminal-emulator,
  with xterm as final fallback.
- `open_terminal_linux()` with correct `-e` / `start --` invocation per terminal.

### Changed
- Default terminal on macOS is now `terminal` (Terminal.app) instead of ghostty.
- `muster-notify` click-to-source supports kitty and wezterm in addition to
  ghostty, alacritty, terminal, and iterm2.
- `resolve_terminal()` is now the single source of truth; `exec_tmux_attach`
  uses it instead of hard-coding a terminal name.

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
