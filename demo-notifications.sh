#!/usr/bin/env bash
#
# Demo: muster session notifications via tmux hooks
#
# On macOS with `muster notifications setup`, notifications appear as native
# desktop banners with click-to-source (opens a new terminal attached to the
# session). Falls back to tmux display-message over SSH or when not installed.
#
# This script creates a session, attaches to it, then triggers notifications
# from a background subshell so you can see the messages appear.

set -euo pipefail

SESSION="muster_demo-notify"
PROFILE_ID="demo-notify"

cleanup() {
    tmux kill-session -t "$SESSION" 2>/dev/null || true
    muster profile delete "$PROFILE_ID" 2>/dev/null || true
}
trap cleanup EXIT

# Kill any leftover from a previous run
tmux kill-session -t "$SESSION" 2>/dev/null || true
muster profile delete "$PROFILE_ID" 2>/dev/null || true

echo "Creating demo session with two tabs..."
muster new "Demo Notify" --tab "Shell:/tmp" --tab "Worker:/tmp" --color "#da70d6" --detach
echo

echo "Hooks installed:"
tmux show-option -t "$SESSION" remain-on-exit
tmux show-hooks -t "$SESSION" 2>/dev/null || true
tmux show-hooks -w -t "$SESSION" 2>/dev/null || true
echo

echo "Attaching to session. Watch for tmux display-messages:"
echo "  1. After 3s  — Worker tab's shell will exit (pane-died notification)"
echo "  2. After 11s — Bell in Shell tab (alert-bell notification)"
echo
echo "Press Enter to attach..."
read -r

# Background subshell: trigger events after delays
(
    sleep 3
    # Trigger pane-died: exit the Worker shell
    tmux send-keys -t "${SESSION}:1" "exit" Enter

    sleep 8
    # Trigger alert-bell: switch to Worker tab first so Shell is background,
    # then send bell to Shell
    tmux select-window -t "${SESSION}:1" 2>/dev/null || true
    tmux send-keys -t "${SESSION}:0" "printf '\\a'" Enter

    # Session stays alive — exit manually with `muster kill demo-notify`
    # or detach with Ctrl-b d
) &

exec muster attach "$PROFILE_ID"
