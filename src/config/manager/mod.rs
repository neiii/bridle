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
    use std::fs;
    use tempfile::TempDir;

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

    fn setup_test_env(temp: &TempDir) {
        let bridle_config_dir = temp.path().join("bridle_config");
        fs::create_dir_all(&bridle_config_dir).unwrap();
        // SAFETY: Tests run single-threaded (--test-threads=1), no concurrent env access
        unsafe { std::env::set_var("BRIDLE_CONFIG_DIR", &bridle_config_dir) };
    }

    #[test]
    fn switch_profile_preserves_edits() {
        let temp = TempDir::new().unwrap();
        setup_test_env(&temp);
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
        setup_test_env(&temp);
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
        setup_test_env(&temp);
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-preserve-unknown", live_config.clone());
        let manager = ProfileManager::new(profiles_dir.clone());

        fs::write(live_config.join("known.txt"), "profile content").unwrap();
        let profile_a = ProfileName::new("profile-a").unwrap();
        manager.create_from_current(&harness, &profile_a).unwrap();

        fs::write(live_config.join("extra.txt"), "extra data").unwrap();
        fs::create_dir_all(live_config.join("extra-dir")).unwrap();
        fs::write(live_config.join("extra-dir/nested.txt"), "nested").unwrap();

        manager.switch_profile(&harness, &profile_a).unwrap();

        assert!(
            live_config.join("extra.txt").exists(),
            "Unknown files should be preserved after switch"
        );
        assert!(
            live_config.join("extra-dir").exists(),
            "Unknown directories should be preserved after switch"
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
        setup_test_env(&temp);
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
        setup_test_env(&temp);
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
        setup_test_env(&temp);
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
}
