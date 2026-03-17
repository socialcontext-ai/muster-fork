# Peek

Check on a session's terminal output without attaching:

```bash
muster peek myproject               # all windows, last 50 lines each
muster peek myproject Shell         # specific window only
muster peek myproject -n 10         # last 10 lines per window
muster peek myproject --json        # machine-readable output
```

Peek uses `tmux capture-pane` to grab scrollback from each window. It's a read-only operation that doesn't affect the session.

## Death Snapshots

When a pane's process exits, muster captures the last 50 lines of output before cleaning up the dead pane. Snapshots are saved to `~/.config/muster/logs/<session_name>/<window_name>.last`.

Files are overwritten on each death event per window name, keeping the directory small. The last few lines are included in the desktop notification body when notifications are enabled.

This preserves output that would otherwise be lost when tmux cleans up dead panes — useful for seeing why a build or server crashed without having been attached at the time.
