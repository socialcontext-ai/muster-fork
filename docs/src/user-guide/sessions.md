# Sessions

Sessions are running tmux sessions managed by muster.

## Starting Sessions

```bash
# From a profile (creates or reattaches)
muster up webapp

# Attach and switch to a specific tab
muster up webapp --tab 2

# Create without attaching
muster up webapp --detach
```

`up` is idempotent — if the session already exists, it attaches. If not, it creates from the profile and attaches.

`up` replaces the current process with `exec tmux attach`. Use `--detach` to create the session in the background without attaching.

## Ad-hoc Sessions

Create a session without a saved profile:

```bash
muster new scratch --tab 'Shell:~/work' --color '#808080'
muster new scratch --detach
```

If `--tab` is omitted, defaults to a single "Shell" tab at `$HOME`.

## Status

```bash
# Show all sessions with tab details
muster status

# List profiles and running sessions
muster list
```

## Inspecting Sessions

```bash
# Show processes running inside sessions
muster ps
muster ps webapp

# Show listening ports
muster ports
muster ports webapp
```

## Changing Colors

```bash
muster color webapp orange
muster color webapp '#22c55e'
muster colour webapp teal-dark   # colour is accepted as an alias
```

The tmux status bar updates instantly and the profile is updated, so the color persists on next `muster up`. If no session is running, updates the profile directly.

See [Colors](colors.md) for the full list of named colors and shade variants.

## Stopping Sessions

```bash
muster down webapp
```

Accepts a profile name, session ID, or full session name. Session metadata dies with the tmux session — no file cleanup needed.

## JSON Output

All commands support `--json` for machine-readable output:

```bash
muster status --json
muster list --json
muster peek webapp --json
```
