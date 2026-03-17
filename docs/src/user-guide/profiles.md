# Profiles

Profiles are saved templates for creating terminal groups. They define the group's name, color, and tab layout.

## Creating Profiles

```bash
# Basic profile with one tab
muster profile save notes --tab 'Shell:~/work/notes' --color '#f97316'

# Multi-tab profile
muster profile save webapp --color '#3b82f6' \
  --tab 'Shell:~/work/app' \
  --tab 'Server:~/work/app:npm run dev' \
  --tab 'Logs:~/work/app/logs'
```

The `--tab` flag uses colon-delimited format: `name:cwd` or `name:cwd:command`. It is repeatable for multiple tabs. If omitted, defaults to a single "Shell" tab at `$HOME`.

## Listing Profiles

```bash
muster profile list
```

## Viewing a Profile

```bash
muster profile show notes
```

## Editing Profiles

### Interactive Editing

Open the profile in your `$EDITOR` as TOML:

```bash
muster profile edit notes
```

### Inline Updates

Update specific fields without opening an editor:

```bash
muster profile update notes --name renamed --color '#22c55e'
```

## Managing Tabs

```bash
# Add a tab
muster profile add-tab notes --name Tests --cwd ~/work/notes --command 'cargo test --watch'

# Remove a tab (by name or 0-based index)
muster profile remove-tab notes Tests
muster profile remove-tab notes 2
```

## Deleting Profiles

```bash
muster profile delete notes
```

This removes the profile from `profiles.json`. It does not affect any running sessions that were launched from this profile.

## Storage

Profiles are stored in `~/.config/muster/profiles.json`:

```json
{
  "profiles": {
    "notes": {
      "id": "notes",
      "name": "notes",
      "color": "#f97316",
      "tabs": [
        { "name": "Shell", "cwd": "/Users/you/work/notes", "command": null }
      ]
    }
  }
}
```

The config directory can be overridden with `--config-dir` or the `MUSTER_CONFIG_DIR` environment variable.
