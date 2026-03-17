# Pin & Unpin

Pin and unpin sync the current tab's state back to the session's profile.

## Pin

```bash
muster pin
```

Run this from inside a muster-managed tmux session. It saves the current tab's name, working directory, and command to the session's profile. This is useful when you've customized a tab at runtime and want those changes persisted.

## Unpin

```bash
muster unpin
```

Removes the current tab from the session's profile. The tab continues to exist in the running session but won't be recreated when the profile is run again.

## Tab Rename Sync

Muster installs a tmux hook that automatically syncs tab renames to the profile when a tab is pinned. This means renaming a tab with `Ctrl-b ,` updates the profile if the tab is pinned.
