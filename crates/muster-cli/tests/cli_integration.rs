//! Integration tests for the muster CLI binary.
//!
//! These tests invoke the `muster` binary via `assert_cmd` and verify its output.
//! Tests that don't require tmux use a temporary config directory with pre-seeded
//! profiles and an isolated tmux socket dir (via TMUX_TMPDIR) so no real sessions
//! are visible.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Create a temp config dir and seed it with a test profile via `profiles.json`.
fn setup_config() -> TempDir {
    let dir = TempDir::new().unwrap();

    let profiles_json = serde_json::json!({
        "profiles": {
            "test-project": {
                "id": "test-project",
                "name": "Test Project",
                "color": "#f97316",
                "tabs": [
                    {
                        "name": "Shell",
                        "cwd": "/tmp",
                        "panes": []
                    },
                    {
                        "name": "Server",
                        "cwd": "/tmp",
                        "command": "echo hello",
                        "panes": []
                    }
                ]
            }
        }
    });
    std::fs::write(
        dir.path().join("profiles.json"),
        serde_json::to_string_pretty(&profiles_json).unwrap(),
    )
    .unwrap();

    dir
}

/// Create a muster command with isolated config dir and tmux socket.
/// Uses TMUX_TMPDIR pointing to an empty dir so tmux finds no server,
/// ensuring tests don't see real sessions.
fn muster_cmd(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("muster").unwrap();
    let tmux_dir = config_dir.path().join("tmux-sock");
    std::fs::create_dir_all(&tmux_dir).unwrap();
    cmd.arg("--config-dir")
        .arg(config_dir.path())
        .env_remove("TMUX")
        .env("TMUX_TMPDIR", &tmux_dir);
    cmd
}

// ---- list command ----

#[test]
fn list_shows_profiles() {
    let dir = setup_config();
    muster_cmd(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Project"))
        .stdout(predicate::str::contains("test-project"));
}

#[test]
fn list_json_output() {
    let dir = setup_config();
    let output = muster_cmd(&dir).args(["--json", "list"]).output().unwrap();
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let profiles = json["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0]["id"], "test-project");
    assert_eq!(profiles[0]["name"], "Test Project");
    assert_eq!(profiles[0]["tabs"].as_array().unwrap().len(), 2);
}

#[test]
fn list_empty_config() {
    let dir = TempDir::new().unwrap();
    muster_cmd(&dir)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No profiles or sessions."));
}

// ---- profile subcommands ----

#[test]
fn profile_list() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Project"))
        .stdout(predicate::str::contains("2 tab(s)"));
}

#[test]
fn profile_list_json() {
    let dir = setup_config();
    let output = muster_cmd(&dir)
        .args(["--json", "profile", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let profiles: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0]["name"], "Test Project");
}

#[test]
fn profile_show() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["profile", "show", "test-project"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Project"))
        .stdout(predicate::str::contains("[0] Shell"))
        .stdout(predicate::str::contains("[1] Server"))
        .stdout(predicate::str::contains("echo hello"));
}

#[test]
fn profile_show_json() {
    let dir = setup_config();
    let output = muster_cmd(&dir)
        .args(["--json", "profile", "show", "test-project"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let profile: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(profile["id"], "test-project");
    assert_eq!(profile["color"], "#f97316");
}

#[test]
fn profile_show_by_name() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["profile", "show", "Test Project"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-project"));
}

#[test]
fn profile_show_not_found() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["profile", "show", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Profile not found: nonexistent"));
}

#[test]
fn profile_save_and_list() {
    let dir = setup_config();

    // Save a new profile
    muster_cmd(&dir)
        .args([
            "profile",
            "save",
            "New Profile",
            "--tab",
            "Main:/tmp",
            "--color",
            "blue",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Saved: New Profile"));

    // Verify it shows up in list
    muster_cmd(&dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("New Profile"))
        .stdout(predicate::str::contains("Test Project"));
}

#[test]
fn profile_save_json() {
    let dir = setup_config();
    let output = muster_cmd(&dir)
        .args([
            "--json",
            "profile",
            "save",
            "JSON Profile",
            "--tab",
            "Shell:/tmp",
            "--color",
            "#aabbcc",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let saved: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(saved["name"], "JSON Profile");
    assert_eq!(saved["id"], "json-profile");
    assert_eq!(saved["color"], "#aabbcc");
}

#[test]
fn profile_delete() {
    let dir = setup_config();

    muster_cmd(&dir)
        .args(["profile", "delete", "test-project"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted: Test Project"));

    // Verify it's gone
    muster_cmd(&dir)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No profiles."));
}

#[test]
fn profile_delete_not_found() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["profile", "delete", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Profile not found"));
}

#[test]
fn profile_add_tab() {
    let dir = setup_config();

    muster_cmd(&dir)
        .args([
            "profile",
            "add-tab",
            "test-project",
            "--name",
            "Logs",
            "--cwd",
            "/var/log",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("3 tab(s)"));

    // Verify the new tab is there
    let output = muster_cmd(&dir)
        .args(["--json", "profile", "show", "test-project"])
        .output()
        .unwrap();
    let profile: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(profile["tabs"].as_array().unwrap().len(), 3);
    assert_eq!(profile["tabs"][2]["name"], "Logs");
}

#[test]
fn profile_remove_tab_by_name() {
    let dir = setup_config();

    muster_cmd(&dir)
        .args(["profile", "remove-tab", "test-project", "Server"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 tab(s)"));
}

#[test]
fn profile_remove_tab_by_index() {
    let dir = setup_config();

    muster_cmd(&dir)
        .args(["profile", "remove-tab", "test-project", "0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 tab(s)"));
}

#[test]
fn profile_remove_last_tab_fails() {
    let dir = setup_config();

    // Remove first tab
    muster_cmd(&dir)
        .args(["profile", "remove-tab", "test-project", "0"])
        .assert()
        .success();

    // Try to remove the last remaining tab
    muster_cmd(&dir)
        .args(["profile", "remove-tab", "test-project", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Cannot remove the last tab"));
}

#[test]
fn profile_update_color() {
    let dir = setup_config();

    muster_cmd(&dir)
        .args(["profile", "update", "test-project", "--color", "red"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated: Test Project"));

    // Verify color changed
    let output = muster_cmd(&dir)
        .args(["--json", "profile", "show", "test-project"])
        .output()
        .unwrap();
    let profile: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    // "red" resolves to a hex value, should differ from original
    assert_ne!(profile["color"], "#f97316");
}

#[test]
fn profile_update_requires_flag() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["profile", "update", "test-project"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--name or --color"));
}

// ---- color command ----

#[test]
fn color_list() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["color", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("red"))
        .stdout(predicate::str::contains("blue"))
        .stdout(predicate::str::contains("Shades:"));
}

#[test]
fn color_list_json() {
    let dir = setup_config();
    let output = muster_cmd(&dir)
        .args(["--json", "color", "--list"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["colors"].as_array().unwrap().len() > 10);
    assert!(json["shades"].as_array().unwrap().len() > 5);
}

// ---- commands that need no sessions ----

#[test]
fn launch_nonexistent_profile() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["launch", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Profile not found"));
}

#[test]
fn kill_nonexistent_session() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["kill", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("session not found"));
}

#[test]
fn status_no_sessions() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No active sessions."));
}

#[test]
fn status_json_no_sessions() {
    let dir = setup_config();
    let output = muster_cmd(&dir)
        .args(["--json", "status"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(sessions.is_empty());
}

#[test]
fn ps_no_sessions() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["ps"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No active sessions."));
}

#[test]
fn top_no_sessions() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["top"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No active sessions."));
}

#[test]
fn ports_no_sessions() {
    let dir = setup_config();
    muster_cmd(&dir)
        .args(["ports"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No active sessions."));
}
