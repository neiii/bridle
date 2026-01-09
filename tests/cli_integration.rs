use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn bridle() -> Command {
    Command::cargo_bin("bridle").unwrap()
}

fn with_isolated_config() -> (Command, TempDir) {
    let temp = TempDir::new().unwrap();
    let mut cmd = bridle();
    cmd.env("BRIDLE_CONFIG_DIR", temp.path());
    (cmd, temp)
}

#[test]
fn help_shows_usage() {
    bridle()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("bridle"));
}

#[test]
fn version_shows_version() {
    bridle()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("bridle"));
}

#[test]
fn profile_list_empty() {
    let (mut cmd, _temp) = with_isolated_config();
    cmd.args(["profile", "list", "opencode"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No profiles found"));
}

#[test]
fn profile_create_and_list() {
    let (mut cmd, temp) = with_isolated_config();

    cmd.args(["profile", "create", "opencode", "test-profile"])
        .assert()
        .success();

    let mut cmd2 = bridle();
    cmd2.env("BRIDLE_CONFIG_DIR", temp.path());
    cmd2.args(["profile", "list", "opencode"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-profile"));
}

#[test]
fn profile_show_not_found() {
    let (mut cmd, _temp) = with_isolated_config();
    cmd.args(["profile", "show", "opencode", "nonexistent"])
        .assert()
        .failure();
}

#[test]
fn profile_create_and_show() {
    let (mut cmd, temp) = with_isolated_config();

    cmd.args(["profile", "create", "opencode", "show-test"])
        .assert()
        .success();

    let mut cmd2 = bridle();
    cmd2.env("BRIDLE_CONFIG_DIR", temp.path());
    cmd2.args(["profile", "show", "opencode", "show-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("show-test"));
}

#[test]
fn crush_profile_show_includes_model() {
    use std::fs;

    let (mut cmd, temp) = with_isolated_config();

    cmd.args(["profile", "create", "crush", "model-test"])
        .assert()
        .success();

    let crush_profile_dir = temp.path().join("profiles/crush/model-test");
    fs::write(
        crush_profile_dir.join("crush.json"),
        r#"{
  "$schema": "https://charm.land/crush.json",
  "model": "gpt-4",
  "mcp": {}
}"#,
    )
    .unwrap();

    let mut cmd2 = bridle();
    cmd2.env("BRIDLE_CONFIG_DIR", temp.path());
    cmd2.args(["profile", "show", "crush", "model-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Model: gpt-4"));
}

#[test]
fn crush_profile_create_from_current_copies_crush_json() {
    use std::fs;

    let temp = TempDir::new().unwrap();
    let bridle_config = temp.path().join("bridle");
    let home_dir = temp.path().join("home");
    let crush_config = home_dir.join(".config/crush");

    fs::create_dir_all(&crush_config).unwrap();
    fs::write(
        crush_config.join("crush.json"),
        r#"{
  "$schema": "https://charm.land/crush.json",
  "model": "gpt-4",
  "mcp": {}
}"#,
    )
    .unwrap();

    let mut cmd = bridle();
    cmd.env("BRIDLE_CONFIG_DIR", &bridle_config);
    cmd.env("HOME", &home_dir);
    cmd.env_remove("XDG_CONFIG_HOME");
    cmd.args([
        "profile",
        "create",
        "crush",
        "from-current",
        "--from-current",
    ])
    .assert()
    .success();

    let profile_crush_json = bridle_config.join("profiles/crush/from-current/crush.json");
    assert!(profile_crush_json.exists());
    let content = fs::read_to_string(profile_crush_json).unwrap();
    assert!(content.contains("\"model\": \"gpt-4\""));
}

#[test]
fn profile_create_and_delete() {
    let (mut cmd, temp) = with_isolated_config();

    cmd.args(["profile", "create", "opencode", "to-delete"])
        .assert()
        .success();

    let mut cmd2 = bridle();
    cmd2.env("BRIDLE_CONFIG_DIR", temp.path());
    cmd2.args(["profile", "delete", "opencode", "to-delete"])
        .assert()
        .success();

    let mut cmd3 = bridle();
    cmd3.env("BRIDLE_CONFIG_DIR", temp.path());
    cmd3.args(["profile", "show", "opencode", "to-delete"])
        .assert()
        .failure();
}

#[test]
fn profile_create_duplicate_fails() {
    let (mut cmd, temp) = with_isolated_config();

    cmd.args(["profile", "create", "opencode", "duplicate"])
        .assert()
        .success();

    let mut cmd2 = bridle();
    cmd2.env("BRIDLE_CONFIG_DIR", temp.path());
    cmd2.args(["profile", "create", "opencode", "duplicate"])
        .assert()
        .failure();
}

#[test]
fn config_get_unknown_setting() {
    let (mut cmd, _temp) = with_isolated_config();
    cmd.args(["config", "get", "nonexistent"])
        .assert()
        .failure();
}

#[test]
fn config_set_and_get() {
    let (mut cmd, temp) = with_isolated_config();

    cmd.args(["config", "set", "profile_marker", "true"])
        .assert()
        .success();

    let mut cmd2 = bridle();
    cmd2.env("BRIDLE_CONFIG_DIR", temp.path());
    cmd2.args(["config", "get", "profile_marker"])
        .assert()
        .success()
        .stdout(predicate::str::contains("true"));
}

#[test]
fn status_shows_harnesses() {
    bridle().arg("status").assert().success();
}

#[test]
fn unknown_harness_fails() {
    let (mut cmd, _temp) = with_isolated_config();
    cmd.args(["profile", "list", "nonexistent-harness"])
        .assert()
        .failure();
}

#[test]
fn profile_switch_preserves_unknown_files() {
    use std::fs;

    let temp = TempDir::new().unwrap();
    let bridle_config = temp.path().join("bridle");
    let xdg_config = temp.path().join("xdg");
    let opencode_config = xdg_config.join("opencode");

    fs::create_dir_all(&opencode_config).unwrap();
    fs::write(opencode_config.join("opencode.jsonc"), "{}").unwrap();

    let mut cmd = bridle();
    cmd.env("BRIDLE_CONFIG_DIR", &bridle_config);
    cmd.env("XDG_CONFIG_HOME", &xdg_config);
    cmd.args([
        "profile",
        "create",
        "opencode",
        "test-switch",
        "--from-current",
    ])
    .assert()
    .success();

    fs::write(opencode_config.join("unknown.txt"), "precious data").unwrap();
    fs::create_dir_all(opencode_config.join("unknown-dir")).unwrap();
    fs::write(
        opencode_config.join("unknown-dir/nested.txt"),
        "nested precious",
    )
    .unwrap();

    let mut cmd2 = bridle();
    cmd2.env("BRIDLE_CONFIG_DIR", &bridle_config);
    cmd2.env("XDG_CONFIG_HOME", &xdg_config);
    cmd2.args(["profile", "switch", "opencode", "test-switch"])
        .assert()
        .success();

    assert!(
        opencode_config.join("unknown.txt").exists(),
        "Unknown file should be preserved after switch"
    );
    assert_eq!(
        fs::read_to_string(opencode_config.join("unknown.txt")).unwrap(),
        "precious data"
    );
    assert!(
        opencode_config.join("unknown-dir/nested.txt").exists(),
        "Unknown nested file should be preserved after switch"
    );
    assert_eq!(
        fs::read_to_string(opencode_config.join("unknown-dir/nested.txt")).unwrap(),
        "nested precious"
    );
    assert!(
        opencode_config.join("opencode.jsonc").exists(),
        "Profile content should still be applied"
    );
}
