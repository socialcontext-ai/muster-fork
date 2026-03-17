# Testing

## Unit Tests

Unit tests do not require tmux:

```bash
# With cargo-nextest
cargo nextest run

# With cargo test
cargo test
```

## Integration Tests

Integration tests create real tmux sessions and require tmux to be installed:

```bash
# With cargo-nextest
cargo nextest run --run-ignored all

# With cargo test
cargo test -- --ignored
```

Integration tests create sessions with unique names and clean up after themselves. They do not interfere with your personal tmux sessions.

## What's Tested

**Unit tests (no tmux required):**
- Profile CRUD (reads/writes JSON files in a temp directory)
- Color computation (hex parsing, dimming, tmux style string generation)
- Session name convention (encoding/decoding profile IDs)
- Control mode stream parser (given raw control mode output, verify parsed events)

**Integration tests (tmux required):**
- Session lifecycle: create from profile, verify tabs exist, destroy
- Tab operations: add, close, rename, verify via tmux queries
- Theme application: set color, verify tmux options
- Control mode: connect, receive events on window add/close
