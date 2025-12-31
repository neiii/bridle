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
        .stdout(predicate::str::contains("[]"));
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
