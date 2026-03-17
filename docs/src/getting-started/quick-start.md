# Quick Start

## Create a Profile

Save a profile for a project with one or more tabs:

```bash
# Single-tab profile
muster profile save notes --tab 'Shell:~/work/notes' --color '#f97316'

# Multi-tab profile
muster profile save webapp --color '#3b82f6' \
  --tab 'Shell:~/work/app' \
  --tab 'Server:~/work/app:npm start' \
  --tab 'Logs:~/work/app/logs'
```

## Launch a Session

```bash
muster launch notes
```

This creates the tmux session and drops you in. You're now inside tmux — detach with `Ctrl-b d` to return to your shell.

If the session already exists, `launch` reattaches instead of creating a duplicate.

## Check What's Running

From another terminal:

```bash
muster status
```

## Reattach

```bash
# By profile name
muster launch notes

# By session name directly
muster attach muster_notes
```

## Ad-hoc Sessions

Create a quick throwaway session without saving a profile:

```bash
muster new scratch
```

## Background Sessions

Create without attaching:

```bash
muster launch notes --detach
muster new scratch --detach
```

## Modify Profiles

```bash
# Add a tab to an existing profile
muster profile add-tab notes --name Editor --cwd ~/work/notes

# Edit the full profile in $EDITOR
muster profile edit notes
```

## Typical Workflow

1. **`muster profile save`** — define a project (name, tabs, color)
2. **`muster launch <name>`** — start or reattach (execs `tmux attach`, replacing your shell)
3. Work inside tmux. Use `Ctrl-b d` to detach back to your regular shell.
4. **`muster launch <name>`** again to reattach later
5. **`muster status`** from another terminal to see all sessions
6. **`muster kill <name>`** when done
