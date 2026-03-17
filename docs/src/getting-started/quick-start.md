# Quick Start

## Create a Profile

Save a profile for a project with one or more tabs:

```bash
# Single-tab profile
muster profile save myproject --tab 'Shell:~/work/myproject' --color '#f97316'

# Multi-tab profile
muster profile save webapp --color '#3b82f6' \
  --tab 'Shell:~/work/app' \
  --tab 'Server:~/work/app:npm start' \
  --tab 'Logs:~/work/app/logs'
```

## Start a Session

```bash
muster up myproject
```

This creates the tmux session and drops you in. You're now inside tmux — detach with `Ctrl-b d` to return to your shell.

If the session already exists, `up` reattaches instead of creating a duplicate.

## Check What's Running

From another terminal:

```bash
muster status
```

## Reattach

```bash
# By profile name
muster up myproject

# By session name directly
muster attach muster_myproject
```

## Ad-hoc Sessions

Create a quick throwaway session without saving a profile:

```bash
muster new scratch
```

## Background Sessions

Create without attaching:

```bash
muster up myproject --detach
muster new scratch --detach
```

## Modify Profiles

```bash
# Add a tab to an existing profile
muster profile add-tab myproject --name Editor --cwd ~/work/myproject

# Edit the full profile in $EDITOR
muster profile edit myproject
```

## Typical Workflow

1. **`muster profile save`** — define a project (name, tabs, color)
2. **`muster up <name>`** — start or reattach (execs `tmux attach`, replacing your shell)
3. Work inside tmux. Use `Ctrl-b d` to detach back to your regular shell.
4. **`muster up <name>`** again to reattach later
5. **`muster status`** from another terminal to see all sessions
6. **`muster down <name>`** when done
