//! Profile management for harness configurations.
//!
//! This module provides [`ProfileManager`], the central coordinator for all profile
//! operations including creation, deletion, switching, and configuration extraction.

mod extraction;
mod files;
mod lifecycle;

use std::path::PathBuf;

use harness_locate::{Harness, InstallationStatus};

use super::BridleConfig;
use super::profile_name::ProfileName;
use super::types::ProfileInfo;
use crate::error::{Error, Result};
use crate::harness::HarnessConfig;

/// Manages harness configuration profiles.
///
/// `ProfileManager` handles the lifecycle of profiles stored under `~/.config/bridle/profiles/`.
/// Each profile is a directory containing configuration files that can be switched into a
/// harness's live configuration directory.
///
/// # Directory Structure
///
/// ```text
/// ~/.config/bridle/profiles/
/// ├── opencode/
/// │   ├── default/
/// │   └── work/
/// ├── claude-code/
/// │   └── default/
/// └── goose/
///     └── default/
/// ```
#[derive(Debug)]
pub struct ProfileManager {
    profiles_dir: PathBuf,
}

const MARKER_PREFIX: &str = "BRIDLE_PROFILE_";

impl ProfileManager {
    /// Creates a new profile manager with the given profiles directory.
    pub fn new(profiles_dir: PathBuf) -> Self {
        Self { profiles_dir }
    }

    fn delete_marker_files(dir: &std::path::Path) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let dominated_name = entry.file_name();
            let Some(name) = dominated_name.to_str() else {
                continue;
            };
            if name.starts_with(MARKER_PREFIX) && entry.file_type()?.is_file() {
                std::fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    fn create_marker_file(dir: &std::path::Path, profile_name: &str) -> Result<()> {
        let marker_path = dir.join(format!("{}{}", MARKER_PREFIX, profile_name));
        std::fs::File::create(marker_path)?;
        Ok(())
    }

    /// Returns the base directory where all profiles are stored.
    pub fn profiles_dir(&self) -> &PathBuf {
        &self.profiles_dir
    }

    /// Returns the filesystem path for a specific profile.
    pub fn profile_path(&self, harness: &dyn HarnessConfig, name: &ProfileName) -> PathBuf {
        self.profiles_dir.join(harness.id()).join(name.as_str())
    }

    /// Checks if a profile exists on disk.
    pub fn profile_exists(&self, harness: &dyn HarnessConfig, name: &ProfileName) -> bool {
        self.profile_path(harness, name).is_dir()
    }

    /// Lists all profiles for a harness, sorted alphabetically.
    ///
    /// # Errors
    /// Returns an error if the profiles directory cannot be read.
    pub fn list_profiles(&self, harness: &dyn HarnessConfig) -> Result<Vec<ProfileName>> {
        let harness_dir = self.profiles_dir.join(harness.id());

        if !harness_dir.exists() {
            return Ok(Vec::new());
        }

        let mut profiles = Vec::new();
        for entry in std::fs::read_dir(&harness_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && let Some(name) = entry.file_name().to_str()
                && let Ok(profile_name) = ProfileName::new(name)
            {
                profiles.push(profile_name);
            }
        }

        profiles.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        Ok(profiles)
    }

    /// Creates an empty profile directory.
    ///
    /// # Errors
    /// Returns [`Error::ProfileExists`] if profile already exists, or IO error on failure.
    pub fn create_profile(
        &self,
        harness: &dyn HarnessConfig,
        name: &ProfileName,
    ) -> Result<PathBuf> {
        let path = self.profile_path(harness, name);

        if path.exists() {
            return Err(Error::ProfileExists(name.as_str().to_string()));
        }

        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    /// Creates a profile by copying the harness's current configuration.
    ///
    /// # Errors
    /// Returns [`Error::ProfileExists`] if profile exists, or IO error on copy failure.
    pub fn create_from_current(
        &self,
        harness: &dyn HarnessConfig,
        name: &ProfileName,
    ) -> Result<PathBuf> {
        self.create_from_current_with_resources(harness, None, name)
    }

    /// Creates a profile from current config, optionally including resource directories.
    ///
    /// # Errors
    /// Returns error if profile exists or copy fails.
    pub fn create_from_current_with_resources(
        &self,
        harness: &dyn HarnessConfig,
        harness_for_resources: Option<&Harness>,
        name: &ProfileName,
    ) -> Result<PathBuf> {
        let profile_path = self.create_profile(harness, name)?;
        files::copy_config_files(harness, true, &profile_path)?;
        if let Some(h) = harness_for_resources {
            files::copy_resource_directories(h, true, &profile_path)?;
        }

        if let Ok(mut config) = BridleConfig::load() {
            config.set_active_profile(harness.id(), name.as_str());
            let _ = config.save();
        }

        Ok(profile_path)
    }

    /// Creates a "default" profile from current harness config if it doesn't exist.
    ///
    /// Returns `Ok(true)` if profile was created, `Ok(false)` if it already existed
    /// or if the harness is not fully installed.
    ///
    /// Only creates for `FullyInstalled` harnesses (both binary and config exist).
    pub fn create_from_current_if_missing(&self, harness: &dyn HarnessConfig) -> Result<bool> {
        let status = harness.installation_status()?;
        if !matches!(status, InstallationStatus::FullyInstalled { .. }) {
            return Ok(false);
        }

        let name = ProfileName::new("default").expect("'default' is a valid profile name");
        if self.profile_exists(harness, &name) {
            return Ok(false);
        }

        self.create_from_current(harness, &name)?;
        Ok(true)
    }

    /// Deletes a profile and all its contents.
    ///
    /// # Errors
    /// Returns [`Error::ProfileNotFound`] if profile doesn't exist.
    pub fn delete_profile(&self, harness: &dyn HarnessConfig, name: &ProfileName) -> Result<()> {
        let path = self.profile_path(harness, name);

        if !path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        std::fs::remove_dir_all(&path)?;
        Ok(())
    }

    /// Extracts and returns detailed information about a profile.
    ///
    /// When a profile is active, reads from the live harness config directory
    /// to reflect any manual edits the user may have made.
    ///
    /// # Errors
    /// Returns [`Error::ProfileNotFound`] if profile doesn't exist.
    pub fn show_profile(&self, harness: &Harness, name: &ProfileName) -> Result<ProfileInfo> {
        let profile_path = self.profile_path(harness, name);

        if !profile_path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        let harness_id = harness.id().to_string();
        let is_active = BridleConfig::load()
            .map(|c| c.active_profile_for(&harness_id) == Some(name.as_str()))
            .unwrap_or(false);

        let live_harness_path = harness.config_dir().unwrap_or(profile_path.clone());
        let extraction_path = if is_active {
            live_harness_path
        } else {
            profile_path.clone()
        };

        let theme = extraction::extract_theme(harness, &extraction_path);
        let model = extraction::extract_model(harness, &extraction_path);

        let mut extraction_errors = Vec::new();

        let mcp_servers = match extraction::extract_mcp_servers(harness, &extraction_path) {
            Ok(servers) => servers,
            Err(e) => {
                extraction_errors.push(format!("MCP config: {}", e));
                Vec::new()
            }
        };

        let (skills, err) = extraction::extract_skills(harness, &extraction_path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        let (commands, err) = extraction::extract_commands(harness, &extraction_path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        let (plugins, err) = extraction::extract_plugins(harness, &extraction_path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        let (agents, err) = extraction::extract_agents(harness, &extraction_path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        let (rules_file, err) = extraction::extract_rules_file(harness, &extraction_path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        Ok(ProfileInfo {
            name: name.as_str().to_string(),
            harness_id,
            is_active,
            path: profile_path,
            mcp_servers,
            skills,
            commands,
            plugins,
            agents,
            rules_file,
            theme,
            model,
            extraction_errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::extraction::{
        DirectoryStructure, extract_resource_summary, list_files_matching, list_subdirs_with_file,
    };
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct TestEnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        prev: Option<OsString>,
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            if let Some(prev) = &self.prev {
                unsafe { std::env::set_var("BRIDLE_CONFIG_DIR", prev) };
            } else {
                unsafe { std::env::remove_var("BRIDLE_CONFIG_DIR") };
            }
        }
    }

    struct MockHarness {
        id: String,
        config_dir: PathBuf,
        mcp_path: Option<PathBuf>,
    }

    impl MockHarness {
        fn new(id: &str, config_dir: PathBuf) -> Self {
            Self {
                id: id.to_string(),
                config_dir,
                mcp_path: None,
            }
        }

        fn with_mcp(mut self, mcp_path: PathBuf) -> Self {
            self.mcp_path = Some(mcp_path);
            self
        }
    }

    impl HarnessConfig for MockHarness {
        fn id(&self) -> &str {
            &self.id
        }

        fn config_dir(&self) -> Result<PathBuf> {
            Ok(self.config_dir.clone())
        }

        fn installation_status(&self) -> Result<InstallationStatus> {
            Ok(InstallationStatus::FullyInstalled {
                binary_path: PathBuf::from("/bin/mock"),
                config_path: self.config_dir.clone(),
            })
        }

        fn mcp_filename(&self) -> Option<String> {
            None
        }

        fn mcp_config_path(&self) -> Option<PathBuf> {
            self.mcp_path.clone()
        }

        fn parse_mcp_servers(
            &self,
            _content: &str,
            _filename: &str,
        ) -> Result<Vec<(String, bool)>> {
            Ok(vec![])
        }
    }

    fn setup_test_env(temp: &TempDir) -> TestEnvGuard {
        let lock = TEST_ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let prev = std::env::var_os("BRIDLE_CONFIG_DIR");
        let bridle_config_dir = temp.path().join("bridle_config");
        fs::create_dir_all(&bridle_config_dir).unwrap();
        unsafe { std::env::set_var("BRIDLE_CONFIG_DIR", &bridle_config_dir) };

        TestEnvGuard { _lock: lock, prev }
    }

    #[test]
    fn switch_profile_preserves_edits() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-preserves-edits", live_config.clone());
        let manager = ProfileManager::new(profiles_dir);

        let profile_a = ProfileName::new("profile-a").unwrap();
        let profile_b = ProfileName::new("profile-b").unwrap();

        fs::write(live_config.join("initial.txt"), "initial").unwrap();
        manager.create_from_current(&harness, &profile_a).unwrap();

        fs::write(live_config.join("initial.txt"), "different").unwrap();
        manager.create_from_current(&harness, &profile_b).unwrap();

        manager.switch_profile(&harness, &profile_a).unwrap();
        assert_eq!(
            fs::read_to_string(live_config.join("initial.txt")).unwrap(),
            "initial"
        );

        fs::write(live_config.join("edited.txt"), "user edit").unwrap();

        manager.switch_profile(&harness, &profile_b).unwrap();
        assert_eq!(
            fs::read_to_string(live_config.join("initial.txt")).unwrap(),
            "different"
        );

        manager.switch_profile(&harness, &profile_a).unwrap();

        assert!(
            live_config.join("edited.txt").exists(),
            "Edit should be preserved"
        );
        assert_eq!(
            fs::read_to_string(live_config.join("edited.txt")).unwrap(),
            "user edit"
        );
    }

    #[test]
    fn create_from_current_copies_mcp_config() {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        let mcp_file = temp.path().join(".mcp.json");

        fs::create_dir_all(&live_config).unwrap();
        fs::write(live_config.join("config.txt"), "config content").unwrap();
        fs::write(&mcp_file, r#"{"servers": {}}"#).unwrap();

        let harness = MockHarness::new("test-copies-mcp", live_config).with_mcp(mcp_file.clone());
        let manager = ProfileManager::new(profiles_dir);

        let profile_name = ProfileName::new("test-profile").unwrap();
        let profile_path = manager
            .create_from_current(&harness, &profile_name)
            .unwrap();

        assert!(profile_path.join("config.txt").exists());
        assert!(profile_path.join(".mcp.json").exists());
        assert_eq!(
            fs::read_to_string(profile_path.join(".mcp.json")).unwrap(),
            r#"{"servers": {}}"#
        );
    }

    #[test]
    fn switch_profile_restores_mcp_config() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        let mcp_file = temp.path().join(".mcp.json");

        fs::create_dir_all(&live_config).unwrap();
        fs::write(live_config.join("config.txt"), "config A").unwrap();
        fs::write(&mcp_file, r#"{"servers": {"a": true}}"#).unwrap();

        let harness =
            MockHarness::new("test-restores-mcp", live_config.clone()).with_mcp(mcp_file.clone());
        let manager = ProfileManager::new(profiles_dir);

        let profile_a = ProfileName::new("profile-a").unwrap();
        manager.create_from_current(&harness, &profile_a).unwrap();

        fs::write(live_config.join("config.txt"), "config B").unwrap();
        fs::write(&mcp_file, r#"{"servers": {"b": true}}"#).unwrap();

        let profile_b = ProfileName::new("profile-b").unwrap();
        manager.create_from_current(&harness, &profile_b).unwrap();

        manager.switch_profile(&harness, &profile_a).unwrap();

        assert_eq!(
            fs::read_to_string(live_config.join("config.txt")).unwrap(),
            "config A"
        );
        assert_eq!(
            fs::read_to_string(&mcp_file).unwrap(),
            r#"{"servers": {"a": true}}"#
        );
    }

    #[test]
    fn switch_preserves_unknown_files() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-preserve-unknown", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Create two profiles
        fs::write(live_config.join("known.txt"), "profile content").unwrap();
        let profile_a = ProfileName::new("profile-a").unwrap();
        let profile_b = ProfileName::new("profile-b").unwrap();
        manager.create_from_current(&harness, &profile_a).unwrap();
        manager.create_from_current(&harness, &profile_b).unwrap();

        // Activate profile-a
        manager.switch_profile(&harness, &profile_a).unwrap();

        // Add extra files while profile-a is active
        fs::write(live_config.join("extra.txt"), "extra data").unwrap();
        fs::create_dir_all(live_config.join("extra-dir")).unwrap();
        fs::write(live_config.join("extra-dir/nested.txt"), "nested").unwrap();

        // Switch to profile-b (saves current state including extra files to profile-a)
        manager.switch_profile(&harness, &profile_b).unwrap();

        // Verify extra files are NOT in harness (full isolation - profile-b doesn't have them)
        assert!(
            !live_config.join("extra.txt").exists(),
            "Extra files should not exist after switching to profile-b"
        );

        // Switch back to profile-a
        manager.switch_profile(&harness, &profile_a).unwrap();

        // Now extra files should be restored (they were saved to profile-a)
        assert!(
            live_config.join("extra.txt").exists(),
            "Unknown files should be restored after switching back to profile-a"
        );
        assert!(
            live_config.join("extra-dir").exists(),
            "Unknown directories should be restored after switching back"
        );
        assert!(
            live_config.join("known.txt").exists(),
            "Profile content should be applied"
        );
    }

    #[test]
    fn save_to_profile_captures_everything() {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-save-all", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        fs::write(live_config.join("config.txt"), "config").unwrap();
        fs::create_dir_all(live_config.join("runtime-dir/nested")).unwrap();
        fs::write(live_config.join("runtime-dir/data.txt"), "runtime").unwrap();
        fs::write(live_config.join("runtime-dir/nested/deep.txt"), "deep").unwrap();

        let profile = ProfileName::new("full-backup").unwrap();
        manager.create_from_current(&harness, &profile).unwrap();

        let profile_path = profiles_dir.join("test-save-all/full-backup");
        assert!(profile_path.join("config.txt").exists());
        assert!(profile_path.join("runtime-dir/data.txt").exists());
        assert!(profile_path.join("runtime-dir/nested/deep.txt").exists());
        assert_eq!(
            fs::read_to_string(profile_path.join("runtime-dir/nested/deep.txt")).unwrap(),
            "deep"
        );
    }

    #[test]
    fn create_from_current_captures_arbitrary_directories() {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        fs::write(live_config.join("config.json"), "{}").unwrap();
        fs::create_dir_all(live_config.join("custom-dir/level2/level3")).unwrap();
        fs::write(live_config.join("custom-dir/data.txt"), "custom data").unwrap();
        fs::write(live_config.join("custom-dir/level2/nested.txt"), "nested").unwrap();
        fs::write(
            live_config.join("custom-dir/level2/level3/deep.txt"),
            "deep",
        )
        .unwrap();

        let harness = MockHarness::new("test-captures-dirs", live_config.clone());
        let manager = ProfileManager::new(profiles_dir);

        let profile = ProfileName::new("test-profile").unwrap();
        let profile_path = manager.create_from_current(&harness, &profile).unwrap();

        assert!(
            profile_path.join("custom-dir").exists(),
            "Arbitrary directory should be captured in profile"
        );
        assert!(
            profile_path.join("custom-dir/data.txt").exists(),
            "Files inside arbitrary directory should be captured"
        );
        assert!(
            profile_path
                .join("custom-dir/level2/level3/deep.txt")
                .exists(),
            "Deep nested files should be captured"
        );
        assert_eq!(
            fs::read_to_string(profile_path.join("custom-dir/level2/level3/deep.txt")).unwrap(),
            "deep"
        );
    }

    #[test]
    fn switch_saves_new_directories_to_old_profile() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-saves-new-dirs", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        fs::write(live_config.join("config.json"), "A").unwrap();
        let profile_a = ProfileName::new("profile-a").unwrap();
        manager.create_from_current(&harness, &profile_a).unwrap();

        fs::write(live_config.join("config.json"), "B").unwrap();
        let profile_b = ProfileName::new("profile-b").unwrap();
        manager.create_from_current(&harness, &profile_b).unwrap();

        manager.switch_profile(&harness, &profile_a).unwrap();

        fs::create_dir_all(live_config.join("new-dir/nested")).unwrap();
        fs::write(live_config.join("new-dir/nested/data.txt"), "new data").unwrap();

        manager.switch_profile(&harness, &profile_b).unwrap();

        let profile_a_path = profiles_dir.join("test-saves-new-dirs").join("profile-a");
        assert!(
            profile_a_path.join("new-dir/nested/data.txt").exists(),
            "New directories added while on profile-a should be saved when switching away"
        );
        assert_eq!(
            fs::read_to_string(profile_a_path.join("new-dir/nested/data.txt")).unwrap(),
            "new data"
        );
    }

    #[test]
    fn deep_nesting_survives_multiple_round_trips() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-deep-nesting", live_config.clone());
        let manager = ProfileManager::new(profiles_dir);

        fs::create_dir_all(live_config.join("a/b/c/d/e/f")).unwrap();
        fs::write(live_config.join("a/b/c/d/e/f/deep.txt"), "level 6").unwrap();
        fs::write(live_config.join("a/b/c/mid.txt"), "level 3").unwrap();
        fs::write(live_config.join("a/shallow.txt"), "level 1").unwrap();

        let profile_a = ProfileName::new("profile-a").unwrap();
        manager.create_from_current(&harness, &profile_a).unwrap();

        fs::write(live_config.join("config.txt"), "B").unwrap();
        let profile_b = ProfileName::new("profile-b").unwrap();
        manager.create_from_current(&harness, &profile_b).unwrap();

        for _ in 0..3 {
            manager.switch_profile(&harness, &profile_a).unwrap();
            manager.switch_profile(&harness, &profile_b).unwrap();
        }
        manager.switch_profile(&harness, &profile_a).unwrap();

        assert!(
            live_config.join("a/b/c/d/e/f/deep.txt").exists(),
            "Deep nested file should survive multiple round trips"
        );
        assert_eq!(
            fs::read_to_string(live_config.join("a/b/c/d/e/f/deep.txt")).unwrap(),
            "level 6"
        );
        assert_eq!(
            fs::read_to_string(live_config.join("a/b/c/mid.txt")).unwrap(),
            "level 3"
        );
        assert_eq!(
            fs::read_to_string(live_config.join("a/shallow.txt")).unwrap(),
            "level 1"
        );
    }

    #[test]
    fn wide_directory_structure_preserved() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-wide-dirs", live_config.clone());
        let manager = ProfileManager::new(profiles_dir);

        for i in 0..10 {
            fs::create_dir_all(live_config.join(format!("dir-{}/sub", i))).unwrap();
            fs::write(
                live_config.join(format!("dir-{}/file.txt", i)),
                format!("data-{}", i),
            )
            .unwrap();
            fs::write(
                live_config.join(format!("dir-{}/sub/nested.txt", i)),
                format!("nested-{}", i),
            )
            .unwrap();
        }

        let profile_a = ProfileName::new("profile-a").unwrap();
        manager.create_from_current(&harness, &profile_a).unwrap();

        fs::write(live_config.join("other.txt"), "other").unwrap();
        let profile_b = ProfileName::new("profile-b").unwrap();
        manager.create_from_current(&harness, &profile_b).unwrap();

        manager.switch_profile(&harness, &profile_b).unwrap();
        manager.switch_profile(&harness, &profile_a).unwrap();

        for i in 0..10 {
            assert!(
                live_config.join(format!("dir-{}/file.txt", i)).exists(),
                "dir-{}/file.txt should exist after round trip",
                i
            );
            assert_eq!(
                fs::read_to_string(live_config.join(format!("dir-{}/file.txt", i))).unwrap(),
                format!("data-{}", i)
            );
            assert!(
                live_config
                    .join(format!("dir-{}/sub/nested.txt", i))
                    .exists(),
                "dir-{}/sub/nested.txt should exist after round trip",
                i
            );
        }
    }

    #[test]
    fn list_files_matching_finds_files_with_extension() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();

        fs::write(dir.join("skill1.md"), "content").unwrap();
        fs::write(dir.join("skill2.md"), "content").unwrap();
        fs::write(dir.join("readme.txt"), "content").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();

        let result = list_files_matching(dir, "*.md");

        assert_eq!(result, vec!["skill1", "skill2"]);
    }

    #[test]
    fn list_subdirs_with_file_finds_matching_dirs() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();

        fs::create_dir_all(dir.join("cmd1")).unwrap();
        fs::write(dir.join("cmd1").join("index.md"), "content").unwrap();

        fs::create_dir_all(dir.join("cmd2")).unwrap();
        fs::write(dir.join("cmd2").join("index.md"), "content").unwrap();

        fs::create_dir_all(dir.join("empty")).unwrap();

        fs::write(dir.join("file.md"), "content").unwrap();

        let result = list_subdirs_with_file(dir, "*", "index.md");

        assert_eq!(result, vec!["cmd1", "cmd2"]);
    }

    #[test]
    fn extract_resource_summary_handles_nonexistent_dir() {
        let temp = TempDir::new().unwrap();
        let structure = DirectoryStructure::Flat {
            file_pattern: "*.md".to_string(),
        };

        let result = extract_resource_summary(temp.path(), "nonexistent", &structure);

        assert!(!result.directory_exists);
        assert!(result.items.is_empty());
    }

    // ==========================================================================
    // Profile Isolation Tests (GitHub Issue #21)
    //
    // These tests verify that resources (skills, agents, commands) installed to
    // one profile do NOT leak to other profiles during profile switching.
    //
    // Bug: When switching from profile A (with skills) to profile B (empty),
    // the skills remain in the harness config dir. When later switching away
    // from B, those skills get saved TO profile B, contaminating it.
    // ==========================================================================

    /// Test that skills installed to one profile don't leak to another profile
    /// when switching profiles.
    ///
    /// This test reproduces GitHub Issue #21:
    /// 1. Create two empty profiles (default, test)
    /// 2. "Install" a skill while default is active (add to harness dir)
    /// 3. Switch to test profile
    /// 4. Switch back to default (this saves harness state → test profile)
    /// 5. Verify: test profile should NOT have the skill
    #[test]
    fn switch_profile_does_not_leak_skills_to_other_profiles() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-skill-isolation", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Create two empty profiles
        let profile_default = ProfileName::new("default").unwrap();
        let profile_test = ProfileName::new("test").unwrap();

        fs::write(live_config.join("config.json"), "{}").unwrap();
        manager
            .create_from_current(&harness, &profile_default)
            .unwrap();
        manager
            .create_from_current(&harness, &profile_test)
            .unwrap();

        // Activate default profile
        manager.switch_profile(&harness, &profile_default).unwrap();

        // Simulate installing a skill while default is active
        // (skills are stored in harness config dir)
        let skills_dir = live_config.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::create_dir_all(skills_dir.join("algorithmic-art")).unwrap();
        fs::write(
            skills_dir.join("algorithmic-art/SKILL.md"),
            "# Algorithmic Art Skill",
        )
        .unwrap();

        // Verify skill exists in harness dir
        assert!(
            live_config.join("skills/algorithmic-art/SKILL.md").exists(),
            "Skill should be installed in harness dir"
        );

        // Switch to test profile (should save current state to default, load test)
        manager.switch_profile(&harness, &profile_test).unwrap();

        // BUG CHECK: After switching to test, skills should NOT be in harness dir
        // because test profile was empty when created
        assert!(
            !live_config.join("skills").exists(),
            "Skills directory should NOT exist after switching to empty test profile"
        );

        // Switch to another profile and back to test to trigger the leak
        manager.switch_profile(&harness, &profile_default).unwrap();
        manager.switch_profile(&harness, &profile_test).unwrap();

        // CRITICAL: test profile should NOT have acquired skills from default
        let test_profile_path = profiles_dir.join("test-skill-isolation/test");
        assert!(
            !test_profile_path.join("skills").exists(),
            "test profile should NOT have skills directory - skills leaked from default!"
        );
    }

    /// Test that switching to an empty profile clears harness resources.
    ///
    /// When profile B is empty, switching to it should result in an empty
    /// harness config (except for base config files in the profile).
    #[test]
    fn switch_to_empty_profile_clears_harness_resources() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-empty-switch", live_config.clone());
        let manager = ProfileManager::new(profiles_dir);

        // Create profile-a with skills
        fs::write(live_config.join("config.json"), r#"{"name":"a"}"#).unwrap();
        fs::create_dir_all(live_config.join("skills/my-skill")).unwrap();
        fs::write(live_config.join("skills/my-skill/SKILL.md"), "# Skill").unwrap();

        let profile_a = ProfileName::new("profile-a").unwrap();
        manager.create_from_current(&harness, &profile_a).unwrap();

        // Verify profile-a has skills
        let profile_a_path = manager.profile_path(&harness, &profile_a);
        assert!(
            profile_a_path.join("skills/my-skill/SKILL.md").exists(),
            "profile-a should have skills"
        );

        // Create profile-b WITHOUT skills (empty except config)
        fs::remove_dir_all(live_config.join("skills")).unwrap();
        fs::write(live_config.join("config.json"), r#"{"name":"b"}"#).unwrap();

        let profile_b = ProfileName::new("profile-b").unwrap();
        manager.create_from_current(&harness, &profile_b).unwrap();

        // Verify profile-b does NOT have skills
        let profile_b_path = manager.profile_path(&harness, &profile_b);
        assert!(
            !profile_b_path.join("skills").exists(),
            "profile-b should NOT have skills"
        );

        // Now switch to profile-a (which has skills)
        manager.switch_profile(&harness, &profile_a).unwrap();
        assert!(
            live_config.join("skills/my-skill/SKILL.md").exists(),
            "After switching to profile-a, skills should be in harness dir"
        );

        // Switch to profile-b (which is empty)
        manager.switch_profile(&harness, &profile_b).unwrap();

        // BUG: This assertion will fail - skills remain in harness dir
        assert!(
            !live_config.join("skills").exists(),
            "After switching to profile-b (empty), skills should be REMOVED from harness dir"
        );
    }

    /// Test that agents don't leak between profiles.
    #[test]
    fn switch_profile_does_not_leak_agents() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-agent-isolation", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Create profile with agents
        fs::write(live_config.join("config.json"), "{}").unwrap();
        fs::create_dir_all(live_config.join("agents/my-agent")).unwrap();
        fs::write(live_config.join("agents/my-agent/index.md"), "# Agent").unwrap();

        let profile_with_agents = ProfileName::new("with-agents").unwrap();
        manager
            .create_from_current(&harness, &profile_with_agents)
            .unwrap();

        // Create empty profile
        fs::remove_dir_all(live_config.join("agents")).unwrap();
        let profile_empty = ProfileName::new("empty").unwrap();
        manager
            .create_from_current(&harness, &profile_empty)
            .unwrap();

        // Switch to profile with agents
        manager
            .switch_profile(&harness, &profile_with_agents)
            .unwrap();
        assert!(live_config.join("agents").exists());

        // Switch to empty profile
        manager.switch_profile(&harness, &profile_empty).unwrap();

        // Agents should not be in harness dir
        assert!(
            !live_config.join("agents").exists(),
            "Agents should be removed when switching to empty profile"
        );

        // Switch back and forth to trigger potential leak
        manager
            .switch_profile(&harness, &profile_with_agents)
            .unwrap();
        manager.switch_profile(&harness, &profile_empty).unwrap();

        // Empty profile should still be empty
        let empty_profile_path = profiles_dir.join("test-agent-isolation/empty");
        assert!(
            !empty_profile_path.join("agents").exists(),
            "Empty profile should NOT have acquired agents"
        );
    }

    /// Test that commands don't leak between profiles.
    #[test]
    fn switch_profile_does_not_leak_commands() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-cmd-isolation", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Create profile with commands
        fs::write(live_config.join("config.json"), "{}").unwrap();
        fs::create_dir_all(live_config.join("commands/my-cmd")).unwrap();
        fs::write(live_config.join("commands/my-cmd/index.md"), "# Command").unwrap();

        let profile_with_cmds = ProfileName::new("with-cmds").unwrap();
        manager
            .create_from_current(&harness, &profile_with_cmds)
            .unwrap();

        // Create empty profile
        fs::remove_dir_all(live_config.join("commands")).unwrap();
        let profile_empty = ProfileName::new("empty").unwrap();
        manager
            .create_from_current(&harness, &profile_empty)
            .unwrap();

        // Switch to profile with commands
        manager
            .switch_profile(&harness, &profile_with_cmds)
            .unwrap();
        assert!(live_config.join("commands").exists());

        // Switch to empty profile
        manager.switch_profile(&harness, &profile_empty).unwrap();

        // Commands should not be in harness dir
        assert!(
            !live_config.join("commands").exists(),
            "Commands should be removed when switching to empty profile"
        );

        // Verify empty profile wasn't contaminated
        let empty_profile_path = profiles_dir.join("test-cmd-isolation/empty");
        assert!(
            !empty_profile_path.join("commands").exists(),
            "Empty profile should NOT have acquired commands"
        );
    }

    /// Test that multiple resource types don't leak simultaneously.
    /// This simulates a realistic scenario where a profile has skills, agents,
    /// and commands installed.
    #[test]
    fn switch_profile_does_not_leak_multiple_resource_types() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-multi-resource", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Create "full" profile with all resource types
        fs::write(live_config.join("config.json"), r#"{"profile":"full"}"#).unwrap();
        fs::create_dir_all(live_config.join("skills/skill1")).unwrap();
        fs::write(live_config.join("skills/skill1/SKILL.md"), "# Skill").unwrap();
        fs::create_dir_all(live_config.join("agents/agent1")).unwrap();
        fs::write(live_config.join("agents/agent1/index.md"), "# Agent").unwrap();
        fs::create_dir_all(live_config.join("commands/cmd1")).unwrap();
        fs::write(live_config.join("commands/cmd1/index.md"), "# Command").unwrap();

        let profile_full = ProfileName::new("full").unwrap();
        manager
            .create_from_current(&harness, &profile_full)
            .unwrap();

        // Create "minimal" profile with NO resources
        for dir in ["skills", "agents", "commands"] {
            let _ = fs::remove_dir_all(live_config.join(dir));
        }
        fs::write(live_config.join("config.json"), r#"{"profile":"minimal"}"#).unwrap();

        let profile_minimal = ProfileName::new("minimal").unwrap();
        manager
            .create_from_current(&harness, &profile_minimal)
            .unwrap();

        // Switch to full profile
        manager.switch_profile(&harness, &profile_full).unwrap();

        // Verify all resources are present
        assert!(live_config.join("skills").exists(), "skills should exist");
        assert!(live_config.join("agents").exists(), "agents should exist");
        assert!(
            live_config.join("commands").exists(),
            "commands should exist"
        );

        // Switch to minimal profile
        manager.switch_profile(&harness, &profile_minimal).unwrap();

        // ALL resources should be gone
        assert!(
            !live_config.join("skills").exists(),
            "skills should be removed"
        );
        assert!(
            !live_config.join("agents").exists(),
            "agents should be removed"
        );
        assert!(
            !live_config.join("commands").exists(),
            "commands should be removed"
        );

        // Verify minimal profile wasn't contaminated after round-trip
        manager.switch_profile(&harness, &profile_full).unwrap();
        manager.switch_profile(&harness, &profile_minimal).unwrap();

        let minimal_path = profiles_dir.join("test-multi-resource/minimal");
        assert!(
            !minimal_path.join("skills").exists(),
            "minimal profile should NOT have skills"
        );
        assert!(
            !minimal_path.join("agents").exists(),
            "minimal profile should NOT have agents"
        );
        assert!(
            !minimal_path.join("commands").exists(),
            "minimal profile should NOT have commands"
        );
    }

    /// Test profile isolation with OpenCode-style directory naming.
    /// OpenCode uses "skill" (singular) instead of "skills".
    #[test]
    fn switch_profile_isolation_opencode_style() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("opencode-isolation", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // OpenCode uses singular names: skill, agent, command
        fs::write(live_config.join("opencode.jsonc"), "{}").unwrap();
        fs::create_dir_all(live_config.join("skill/algorithmic-art")).unwrap();
        fs::write(live_config.join("skill/algorithmic-art/SKILL.md"), "# Art").unwrap();
        fs::create_dir_all(live_config.join("agent/my-agent")).unwrap();
        fs::write(live_config.join("agent/my-agent/index.md"), "# Agent").unwrap();

        let profile_full = ProfileName::new("full").unwrap();
        manager
            .create_from_current(&harness, &profile_full)
            .unwrap();

        // Create empty profile
        fs::remove_dir_all(live_config.join("skill")).unwrap();
        fs::remove_dir_all(live_config.join("agent")).unwrap();
        fs::write(live_config.join("opencode.jsonc"), "{}").unwrap();

        let profile_empty = ProfileName::new("empty").unwrap();
        manager
            .create_from_current(&harness, &profile_empty)
            .unwrap();

        // Switch full -> empty
        manager.switch_profile(&harness, &profile_full).unwrap();
        manager.switch_profile(&harness, &profile_empty).unwrap();

        // Harness should be clean
        assert!(
            !live_config.join("skill").exists(),
            "OpenCode skill dir should be removed"
        );
        assert!(
            !live_config.join("agent").exists(),
            "OpenCode agent dir should be removed"
        );

        // Empty profile should stay empty
        let empty_path = profiles_dir.join("opencode-isolation/empty");
        assert!(
            !empty_path.join("skill").exists(),
            "Empty profile should NOT have skill dir"
        );
    }

    /// Test profile isolation with Claude Code plugin structure.
    /// Claude Code stores agents/commands inside plugins directory.
    #[test]
    fn switch_profile_isolation_claude_style() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("claude-isolation", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Claude uses: skills/, plugins/<name>/agents/, plugins/<name>/commands/
        fs::write(live_config.join("settings.json"), "{}").unwrap();
        fs::create_dir_all(live_config.join("skills/my-skill")).unwrap();
        fs::write(live_config.join("skills/my-skill/SKILL.md"), "# Skill").unwrap();
        fs::create_dir_all(live_config.join("plugins/bridle/agents/my-agent")).unwrap();
        fs::write(
            live_config.join("plugins/bridle/agents/my-agent/index.md"),
            "# Agent",
        )
        .unwrap();

        let profile_full = ProfileName::new("full").unwrap();
        manager
            .create_from_current(&harness, &profile_full)
            .unwrap();

        // Create empty profile
        fs::remove_dir_all(live_config.join("skills")).unwrap();
        fs::remove_dir_all(live_config.join("plugins")).unwrap();

        let profile_empty = ProfileName::new("empty").unwrap();
        manager
            .create_from_current(&harness, &profile_empty)
            .unwrap();

        // Switch full -> empty
        manager.switch_profile(&harness, &profile_full).unwrap();
        manager.switch_profile(&harness, &profile_empty).unwrap();

        // Harness should be clean
        assert!(
            !live_config.join("skills").exists(),
            "Claude skills dir should be removed"
        );
        assert!(
            !live_config.join("plugins").exists(),
            "Claude plugins dir should be removed"
        );
    }

    /// Test profile isolation with Goose-style directory naming.
    /// Goose uses "skills" directory.
    #[test]
    fn switch_profile_isolation_goose_style() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("goose-isolation", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Goose uses: skills/
        fs::write(live_config.join("config.yaml"), "GOOSE_MODE: auto").unwrap();
        fs::create_dir_all(live_config.join("skills/my-skill")).unwrap();
        fs::write(live_config.join("skills/my-skill/SKILL.md"), "# Skill").unwrap();

        let profile_full = ProfileName::new("full").unwrap();
        manager
            .create_from_current(&harness, &profile_full)
            .unwrap();

        // Create empty profile
        fs::remove_dir_all(live_config.join("skills")).unwrap();

        let profile_empty = ProfileName::new("empty").unwrap();
        manager
            .create_from_current(&harness, &profile_empty)
            .unwrap();

        // Switch full -> empty
        manager.switch_profile(&harness, &profile_full).unwrap();
        manager.switch_profile(&harness, &profile_empty).unwrap();

        // Harness should be clean
        assert!(
            !live_config.join("skills").exists(),
            "Goose skills dir should be removed"
        );

        // Empty profile should stay empty
        let empty_path = profiles_dir.join("goose-isolation/empty");
        assert!(
            !empty_path.join("skills").exists(),
            "Empty profile should NOT have skills"
        );
    }

    /// Comprehensive test to determine WHICH resource types are affected by the leak bug.
    /// Tests: skills, agents, commands, plugins, and MCP config files.
    ///
    /// This test will show exactly which resource types leak and which don't.
    #[test]
    fn comprehensive_resource_leak_verification() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        let mcp_config = temp.path().join("mcp.json");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-comprehensive", live_config.clone())
            .with_mcp(mcp_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Create "full" profile with ALL resource types
        fs::write(live_config.join("config.json"), r#"{"profile":"full"}"#).unwrap();

        // Skills
        fs::create_dir_all(live_config.join("skills/test-skill")).unwrap();
        fs::write(
            live_config.join("skills/test-skill/SKILL.md"),
            "# Test Skill",
        )
        .unwrap();

        // Agents
        fs::create_dir_all(live_config.join("agents/test-agent")).unwrap();
        fs::write(
            live_config.join("agents/test-agent/index.md"),
            "# Test Agent",
        )
        .unwrap();

        // Commands
        fs::create_dir_all(live_config.join("commands/test-cmd")).unwrap();
        fs::write(
            live_config.join("commands/test-cmd/index.md"),
            "# Test Command",
        )
        .unwrap();

        // Plugins
        fs::create_dir_all(live_config.join("plugins/test-plugin")).unwrap();
        fs::write(
            live_config.join("plugins/test-plugin/plugin.json"),
            r#"{"name":"test"}"#,
        )
        .unwrap();

        // MCP config (separate file)
        fs::write(&mcp_config, r#"{"mcpServers":{"test-server":{}}}"#).unwrap();

        let profile_full = ProfileName::new("full").unwrap();
        manager
            .create_from_current(&harness, &profile_full)
            .unwrap();

        // Verify full profile has everything
        let full_path = profiles_dir.join("test-comprehensive/full");
        assert!(full_path.join("skills").exists(), "full should have skills");
        assert!(full_path.join("agents").exists(), "full should have agents");
        assert!(
            full_path.join("commands").exists(),
            "full should have commands"
        );
        assert!(
            full_path.join("plugins").exists(),
            "full should have plugins"
        );
        assert!(full_path.join("mcp.json").exists(), "full should have MCP");

        // Create "empty" profile with NO resources
        for dir in ["skills", "agents", "commands", "plugins"] {
            let _ = fs::remove_dir_all(live_config.join(dir));
        }
        fs::write(live_config.join("config.json"), r#"{"profile":"empty"}"#).unwrap();
        fs::write(&mcp_config, r#"{}"#).unwrap(); // Empty MCP

        let profile_empty = ProfileName::new("empty").unwrap();
        manager
            .create_from_current(&harness, &profile_empty)
            .unwrap();

        // Verify empty profile has nothing
        let empty_path = profiles_dir.join("test-comprehensive/empty");
        assert!(
            !empty_path.join("skills").exists(),
            "empty should NOT have skills initially"
        );
        assert!(
            !empty_path.join("agents").exists(),
            "empty should NOT have agents initially"
        );
        assert!(
            !empty_path.join("commands").exists(),
            "empty should NOT have commands initially"
        );
        assert!(
            !empty_path.join("plugins").exists(),
            "empty should NOT have plugins initially"
        );

        // Switch to full profile (loads all resources into harness dir)
        manager.switch_profile(&harness, &profile_full).unwrap();

        // Verify harness dir has all resources
        assert!(
            live_config.join("skills").exists(),
            "harness should have skills after switching to full"
        );
        assert!(
            live_config.join("agents").exists(),
            "harness should have agents after switching to full"
        );
        assert!(
            live_config.join("commands").exists(),
            "harness should have commands after switching to full"
        );
        assert!(
            live_config.join("plugins").exists(),
            "harness should have plugins after switching to full"
        );

        // Switch to empty profile
        manager.switch_profile(&harness, &profile_empty).unwrap();

        // CHECK: Which resources LEAKED (remain in harness dir)?
        let skills_leaked = live_config.join("skills").exists();
        let agents_leaked = live_config.join("agents").exists();
        let commands_leaked = live_config.join("commands").exists();
        let plugins_leaked = live_config.join("plugins").exists();

        // Now switch back and forth to trigger profile contamination
        manager.switch_profile(&harness, &profile_full).unwrap();
        manager.switch_profile(&harness, &profile_empty).unwrap();

        // CHECK: Which resources CONTAMINATED the empty profile?
        let skills_contaminated = empty_path.join("skills").exists();
        let agents_contaminated = empty_path.join("agents").exists();
        let commands_contaminated = empty_path.join("commands").exists();
        let plugins_contaminated = empty_path.join("plugins").exists();

        // Report findings
        println!("\n=== RESOURCE LEAK VERIFICATION ===");
        println!(
            "Skills:   leaked={:<5} contaminated={}",
            skills_leaked, skills_contaminated
        );
        println!(
            "Agents:   leaked={:<5} contaminated={}",
            agents_leaked, agents_contaminated
        );
        println!(
            "Commands: leaked={:<5} contaminated={}",
            commands_leaked, commands_contaminated
        );
        println!(
            "Plugins:  leaked={:<5} contaminated={}",
            plugins_leaked, plugins_contaminated
        );
        println!("==================================\n");

        // Assert NONE should leak (this will fail, proving the bug scope)
        assert!(
            !skills_leaked,
            "BUG: Skills leaked to harness after switching to empty profile"
        );
        assert!(
            !agents_leaked,
            "BUG: Agents leaked to harness after switching to empty profile"
        );
        assert!(
            !commands_leaked,
            "BUG: Commands leaked to harness after switching to empty profile"
        );
        assert!(
            !plugins_leaked,
            "BUG: Plugins leaked to harness after switching to empty profile"
        );

        // Assert empty profile should NOT be contaminated
        assert!(
            !skills_contaminated,
            "BUG: Skills contaminated empty profile"
        );
        assert!(
            !agents_contaminated,
            "BUG: Agents contaminated empty profile"
        );
        assert!(
            !commands_contaminated,
            "BUG: Commands contaminated empty profile"
        );
        assert!(
            !plugins_contaminated,
            "BUG: Plugins contaminated empty profile"
        );
    }

    /// Test MCP config file leak specifically.
    /// MCP configs are handled separately from directory resources.
    #[test]
    fn mcp_config_does_not_leak_between_profiles() {
        let temp = TempDir::new().unwrap();
        let _env = setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        let mcp_config = temp.path().join("mcp.json");
        fs::create_dir_all(&live_config).unwrap();

        let harness =
            MockHarness::new("test-mcp-leak", live_config.clone()).with_mcp(mcp_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        // Profile with MCP servers
        fs::write(live_config.join("config.json"), "{}").unwrap();
        fs::write(
            &mcp_config,
            r#"{"mcpServers":{"server1":{"command":"cmd1"},"server2":{"command":"cmd2"}}}"#,
        )
        .unwrap();

        let profile_with_mcp = ProfileName::new("with-mcp").unwrap();
        manager
            .create_from_current(&harness, &profile_with_mcp)
            .unwrap();

        // Profile without MCP servers
        fs::write(&mcp_config, r#"{}"#).unwrap();

        let profile_no_mcp = ProfileName::new("no-mcp").unwrap();
        manager
            .create_from_current(&harness, &profile_no_mcp)
            .unwrap();

        // Switch to profile with MCP
        manager.switch_profile(&harness, &profile_with_mcp).unwrap();
        let mcp_content = fs::read_to_string(&mcp_config).unwrap();
        assert!(
            mcp_content.contains("server1"),
            "MCP should have servers after switching to with-mcp"
        );

        // Switch to profile without MCP
        manager.switch_profile(&harness, &profile_no_mcp).unwrap();
        let mcp_content = fs::read_to_string(&mcp_config).unwrap();

        // MCP config should be empty/minimal after switching to no-mcp profile
        assert!(
            !mcp_content.contains("server1"),
            "BUG: MCP servers leaked - server1 should not exist after switching to no-mcp profile"
        );
        assert!(
            !mcp_content.contains("server2"),
            "BUG: MCP servers leaked - server2 should not exist after switching to no-mcp profile"
        );
    }
}
