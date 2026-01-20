use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn bridle() -> Command {
    Command::cargo_bin("bridle").unwrap()
}

fn ensure_fake_opencode_installed(root: &Path) -> (PathBuf, PathBuf) {
    use std::fs;

    let xdg_config_home = root.join("xdg");
    let opencode_config = xdg_config_home.join("opencode");
    fs::create_dir_all(&opencode_config).unwrap();
    fs::write(opencode_config.join("opencode.jsonc"), "{}").unwrap();

    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    #[cfg(unix)]
    {
        let opencode_bin = bin_dir.join("opencode");
        fs::write(&opencode_bin, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&opencode_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(windows)]
    {
        fs::write(bin_dir.join("opencode.cmd"), "@echo off\nexit /b 0\n").unwrap();
    }

    (xdg_config_home, bin_dir)
}

fn ensure_fake_crush_installed(root: &Path) -> (PathBuf, PathBuf) {
    use std::fs;

    let xdg_config_home = root.join("xdg");
    let crush_config = xdg_config_home.join("crush");
    fs::create_dir_all(&crush_config).unwrap();
    fs::write(crush_config.join("crush.json"), "{}").unwrap();

    let bin_dir = root.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    #[cfg(unix)]
    {
        let crush_bin = bin_dir.join("crush");
        fs::write(&crush_bin, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&crush_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(windows)]
    {
        fs::write(bin_dir.join("crush.cmd"), "@echo off\nexit /b 0\n").unwrap();
    }

    (xdg_config_home, bin_dir)
}

fn set_common_env(
    cmd: &mut Command,
    bridle_config_dir: &Path,
    xdg_config_home: &Path,
    bin_dir: &Path,
) {
    cmd.env("BRIDLE_CONFIG_DIR", bridle_config_dir);
    cmd.env("XDG_CONFIG_HOME", xdg_config_home);

    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = std::env::split_paths(&current_path).collect::<Vec<_>>();
    paths.insert(0, bin_dir.to_path_buf());
    let new_path = std::env::join_paths(paths).unwrap();
    cmd.env("PATH", new_path);
}

fn with_isolated_config() -> (Command, TempDir) {
    let temp = TempDir::new().unwrap();
    let (xdg_config_home, bin_dir) = ensure_fake_opencode_installed(temp.path());

    let mut cmd = bridle();
    set_common_env(&mut cmd, temp.path(), &xdg_config_home, &bin_dir);

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

    let (xdg_config_home, bin_dir) = ensure_fake_opencode_installed(temp.path());
    let mut cmd2 = bridle();
    set_common_env(&mut cmd2, temp.path(), &xdg_config_home, &bin_dir);
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

    let (xdg_config_home, bin_dir) = ensure_fake_opencode_installed(temp.path());
    let mut cmd2 = bridle();
    set_common_env(&mut cmd2, temp.path(), &xdg_config_home, &bin_dir);
    cmd2.args(["profile", "show", "opencode", "show-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("show-test"));
}

#[test]
#[ignore = "Requires Crush to be installed (harness-locate doesn't support XDG_CONFIG_HOME override on macOS)"]
fn crush_profile_show_includes_model() {
    use std::fs;

    let temp = TempDir::new().unwrap();
    let (xdg_config_home, bin_dir) = ensure_fake_crush_installed(temp.path());
    let bridle_config = temp.path().join("bridle_config");

    let mut cmd = bridle();
    set_common_env(&mut cmd, &bridle_config, &xdg_config_home, &bin_dir);
    cmd.args(["profile", "create", "crush", "model-test"])
        .assert()
        .success();

    let crush_profile_dir = bridle_config.join("profiles/crush/model-test");
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
    set_common_env(&mut cmd2, &bridle_config, &xdg_config_home, &bin_dir);
    cmd2.args(["profile", "show", "crush", "model-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Model: gpt-4"));
}

#[test]
#[ignore = "Requires Crush to be installed (harness-locate doesn't support XDG_CONFIG_HOME override on macOS)"]
fn crush_profile_create_from_current_copies_crush_json() {
    use std::fs;

    let temp = TempDir::new().unwrap();
    let bridle_config = temp.path().join("bridle");
    let xdg_config_home = temp.path().join("xdg");
    let crush_config = xdg_config_home.join("crush");

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

    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    #[cfg(unix)]
    {
        let crush_bin = bin_dir.join("crush");
        fs::write(&crush_bin, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&crush_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(windows)]
    {
        fs::write(bin_dir.join("crush.cmd"), "@echo off\nexit /b 0\n").unwrap();
    }

    let mut cmd = bridle();
    set_common_env(&mut cmd, &bridle_config, &xdg_config_home, &bin_dir);
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

    let (xdg_config_home, bin_dir) = ensure_fake_opencode_installed(temp.path());
    let mut cmd2 = bridle();
    set_common_env(&mut cmd2, temp.path(), &xdg_config_home, &bin_dir);
    cmd2.args(["profile", "delete", "opencode", "to-delete"])
        .assert()
        .success();

    let mut cmd3 = bridle();
    set_common_env(&mut cmd3, temp.path(), &xdg_config_home, &bin_dir);
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

    let (xdg_config_home, bin_dir) = ensure_fake_opencode_installed(temp.path());
    let mut cmd2 = bridle();
    set_common_env(&mut cmd2, temp.path(), &xdg_config_home, &bin_dir);
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

    let (xdg_config_home, bin_dir) = ensure_fake_opencode_installed(temp.path());
    let mut cmd2 = bridle();
    set_common_env(&mut cmd2, temp.path(), &xdg_config_home, &bin_dir);
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
    let (xdg_config, bin_dir) = ensure_fake_opencode_installed(temp.path());
    let opencode_config = xdg_config.join("opencode");

    let mut cmd = bridle();
    set_common_env(&mut cmd, &bridle_config, &xdg_config, &bin_dir);
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
    set_common_env(&mut cmd2, &bridle_config, &xdg_config, &bin_dir);
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
