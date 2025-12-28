//! Profile management.

use std::path::PathBuf;

use chrono::Local;
use get_harness::{DirectoryStructure, Harness, InstallationStatus, Scope};

use super::BridleConfig;
use super::profile_name::ProfileName;
use crate::error::{Error, Result};
use crate::harness::HarnessConfig;

fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' && !escape_next {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if !in_string && c == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    while let Some(&ch) = chars.peek() {
                        if ch == '\n' {
                            break;
                        }
                        chars.next();
                    }
                }
                Some('*') => {
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => result.push(c),
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// MCP server info with enabled status.
#[derive(Debug, Clone, Default)]
pub struct McpServerInfo {
    /// Server name.
    pub name: String,
    /// Whether the server is enabled.
    pub enabled: bool,
}

/// Summary of directory-based resources (skills, commands, etc.).
#[derive(Debug, Clone, Default)]
pub struct ResourceSummary {
    /// List of resource names/items.
    pub items: Vec<String>,
    /// Whether the resource directory exists.
    pub directory_exists: bool,
}

/// Information about a profile for display purposes.
#[derive(Debug, Clone, Default)]
pub struct ProfileInfo {
    /// Profile name.
    pub name: String,
    /// Harness identifier.
    pub harness_id: String,
    /// Whether this is the currently active profile.
    pub is_active: bool,
    /// Path to the profile directory.
    pub path: PathBuf,

    /// MCP servers with enabled status.
    pub mcp_servers: Vec<McpServerInfo>,

    /// Skills directory summary.
    pub skills: ResourceSummary,
    /// Commands directory summary.
    pub commands: ResourceSummary,
    /// Plugins directory summary (OpenCode only).
    pub plugins: Option<ResourceSummary>,
    /// Agents directory summary (OpenCode only).
    pub agents: Option<ResourceSummary>,
    /// Path to rules file if it exists.
    pub rules_file: Option<PathBuf>,
    /// Theme setting (OpenCode only).
    pub theme: Option<String>,
    /// Model setting.
    pub model: Option<String>,
    /// Errors encountered during extraction.
    pub extraction_errors: Vec<String>,
}

#[derive(Debug)]
pub struct ProfileManager {
    profiles_dir: PathBuf,
}

impl ProfileManager {
    pub fn new(profiles_dir: PathBuf) -> Self {
        Self { profiles_dir }
    }

    pub fn profiles_dir(&self) -> &PathBuf {
        &self.profiles_dir
    }

    pub fn profile_path(&self, harness: &dyn HarnessConfig, name: &ProfileName) -> PathBuf {
        self.profiles_dir.join(harness.id()).join(name.as_str())
    }

    pub fn profile_exists(&self, harness: &dyn HarnessConfig, name: &ProfileName) -> bool {
        self.profile_path(harness, name).is_dir()
    }

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

    pub fn create_from_current(
        &self,
        harness: &dyn HarnessConfig,
        name: &ProfileName,
    ) -> Result<PathBuf> {
        let profile_path = self.create_profile(harness, name)?;
        let source_dir = harness.config_dir()?;

        if !source_dir.exists() {
            return Ok(profile_path);
        }

        for entry in std::fs::read_dir(&source_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let dest = profile_path.join(entry.file_name());
                std::fs::copy(entry.path(), dest)?;
            }
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

    pub fn delete_profile(&self, harness: &dyn HarnessConfig, name: &ProfileName) -> Result<()> {
        let path = self.profile_path(harness, name);

        if !path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        std::fs::remove_dir_all(&path)?;
        Ok(())
    }

    pub fn show_profile(
        &self,
        harness: &dyn HarnessConfig,
        name: &ProfileName,
    ) -> Result<ProfileInfo> {
        let path = self.profile_path(harness, name);

        if !path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        let harness_id = harness.id().to_string();
        let is_active = BridleConfig::load()
            .map(|c| c.active_profile_for(&harness_id) == Some(name.as_str()))
            .unwrap_or(false);

        let mcp_servers = self.extract_mcp_servers(harness, &path)?;
        let theme = self.extract_theme(harness, &path);
        let model = self.extract_model(harness, &path);

        Ok(ProfileInfo {
            name: name.as_str().to_string(),
            harness_id,
            is_active,
            path,
            mcp_servers,
            skills: ResourceSummary::default(),
            commands: ResourceSummary::default(),
            plugins: None,
            agents: None,
            rules_file: None,
            theme,
            model,
            extraction_errors: Vec::new(),
        })
    }

    pub fn show_profile_full(
        &self,
        harness: &Harness,
        harness_config: &dyn HarnessConfig,
        name: &ProfileName,
    ) -> Result<ProfileInfo> {
        let path = self.profile_path(harness_config, name);

        if !path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        let harness_id = harness_config.id().to_string();
        let is_active = BridleConfig::load()
            .map(|c| c.active_profile_for(&harness_id) == Some(name.as_str()))
            .unwrap_or(false);

        let mcp_servers = self.extract_mcp_servers(harness_config, &path)?;
        let theme = self.extract_theme(harness_config, &path);
        let model = self.extract_model(harness_config, &path);

        let mut extraction_errors = Vec::new();

        let (skills, err) = self.extract_skills(harness, &path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        let (commands, err) = self.extract_commands(harness, &path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        let (plugins, err) = self.extract_plugins(harness, &path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        let (agents, err) = self.extract_agents(harness, &path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        let (rules_file, err) = self.extract_rules_file(harness, &path);
        if let Some(e) = err {
            extraction_errors.push(e);
        }

        Ok(ProfileInfo {
            name: name.as_str().to_string(),
            harness_id,
            is_active,
            path,
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

    fn extract_mcp_servers(
        &self,
        harness: &dyn HarnessConfig,
        profile_path: &std::path::Path,
    ) -> Result<Vec<McpServerInfo>> {
        let mcp_filename = match harness.mcp_filename() {
            Some(f) => f,
            None => return Ok(Vec::new()),
        };

        let profile_mcp_path = profile_path.join(mcp_filename);

        if !profile_mcp_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&profile_mcp_path)?;
        let servers = harness.parse_mcp_servers(&content)?;
        Ok(servers
            .into_iter()
            .map(|(name, enabled)| McpServerInfo { name, enabled })
            .collect())
    }

    fn extract_theme(
        &self,
        harness: &dyn HarnessConfig,
        profile_path: &std::path::Path,
    ) -> Option<String> {
        if harness.id() != "opencode" {
            return None;
        }

        let config_path = profile_path.join("opencode.jsonc");
        if !config_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&config_path).ok()?;
        let clean_json = strip_jsonc_comments(&content);
        let parsed: serde_json::Value = serde_json::from_str(&clean_json).ok()?;
        parsed
            .get("theme")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    fn extract_model(
        &self,
        harness: &dyn HarnessConfig,
        profile_path: &std::path::Path,
    ) -> Option<String> {
        match harness.id() {
            "opencode" => self.extract_model_opencode(profile_path),
            "claude-code" => self.extract_model_claude_code(profile_path),
            "goose" => self.extract_model_goose(profile_path),
            _ => None,
        }
    }

    fn extract_model_opencode(&self, profile_path: &std::path::Path) -> Option<String> {
        let config_path = profile_path.join("opencode.jsonc");
        let content = std::fs::read_to_string(&config_path).ok()?;
        let clean_json = strip_jsonc_comments(&content);
        let parsed: serde_json::Value = serde_json::from_str(&clean_json).ok()?;
        parsed
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    fn extract_model_claude_code(&self, profile_path: &std::path::Path) -> Option<String> {
        let config_path = profile_path.join("settings.json");
        let content = std::fs::read_to_string(&config_path).ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
        parsed
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    fn extract_model_goose(&self, profile_path: &std::path::Path) -> Option<String> {
        let config_path = profile_path.join("config.yaml");
        let content = std::fs::read_to_string(&config_path).ok()?;
        let parsed: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
        parsed
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    fn extract_skills(
        &self,
        harness: &Harness,
        profile_path: &std::path::Path,
    ) -> (ResourceSummary, Option<String>) {
        match harness.skills(&Scope::Global) {
            Ok(Some(dir)) => (
                Self::extract_resource_summary(profile_path, "skills", &dir.structure),
                None,
            ),
            Ok(None) => (ResourceSummary::default(), None),
            Err(e) => (ResourceSummary::default(), Some(format!("skills: {}", e))),
        }
    }

    fn extract_commands(
        &self,
        harness: &Harness,
        profile_path: &std::path::Path,
    ) -> (ResourceSummary, Option<String>) {
        match harness.commands(&Scope::Global) {
            Ok(Some(dir)) => (
                Self::extract_resource_summary(profile_path, "commands", &dir.structure),
                None,
            ),
            Ok(None) => (ResourceSummary::default(), None),
            Err(e) => (ResourceSummary::default(), Some(format!("commands: {}", e))),
        }
    }

    fn extract_plugins(
        &self,
        harness: &Harness,
        profile_path: &std::path::Path,
    ) -> (Option<ResourceSummary>, Option<String>) {
        match harness.plugins(&Scope::Global) {
            Ok(Some(dir)) => (
                Some(Self::extract_resource_summary(
                    profile_path,
                    "plugins",
                    &dir.structure,
                )),
                None,
            ),
            Ok(None) => (None, None),
            Err(e) => (None, Some(format!("plugins: {}", e))),
        }
    }

    fn extract_agents(
        &self,
        harness: &Harness,
        profile_path: &std::path::Path,
    ) -> (Option<ResourceSummary>, Option<String>) {
        match harness.agents(&Scope::Global) {
            Ok(Some(dir)) => (
                Some(Self::extract_resource_summary(
                    profile_path,
                    "agents",
                    &dir.structure,
                )),
                None,
            ),
            Ok(None) => (None, None),
            Err(e) => (None, Some(format!("agents: {}", e))),
        }
    }

    fn extract_rules_file(
        &self,
        harness: &Harness,
        profile_path: &std::path::Path,
    ) -> (Option<PathBuf>, Option<String>) {
        match harness.rules(&Scope::Global) {
            Ok(Some(dir)) => {
                let rules_path = match &dir.structure {
                    DirectoryStructure::Flat { file_pattern } => {
                        if file_pattern.contains('*') {
                            Self::find_first_matching_file(profile_path, file_pattern)
                        } else {
                            let path = profile_path.join(file_pattern);
                            if path.exists() { Some(path) } else { None }
                        }
                    }
                    DirectoryStructure::Nested { file_name, .. } => {
                        let path = profile_path.join(file_name);
                        if path.exists() { Some(path) } else { None }
                    }
                };
                (rules_path, None)
            }
            Ok(None) => (None, None),
            Err(e) => (None, Some(format!("rules: {}", e))),
        }
    }

    fn find_first_matching_file(dir: &std::path::Path, pattern: &str) -> Option<PathBuf> {
        let mut matches: Vec<PathBuf> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.path())
            .filter(|p| Self::matches_pattern(p.file_name().and_then(|n| n.to_str()), pattern))
            .collect();
        matches.sort();
        matches.into_iter().next()
    }

    fn matches_pattern(filename: Option<&str>, pattern: &str) -> bool {
        let Some(name) = filename else { return false };
        if pattern == "*" {
            return true;
        }
        if let Some(suffix) = pattern.strip_prefix("*.") {
            return name.ends_with(&format!(".{}", suffix));
        }
        if let Some(suffix) = pattern.strip_prefix('*') {
            return name.ends_with(suffix);
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return name.starts_with(prefix);
        }
        name == pattern
    }

    fn extract_resource_summary(
        base_path: &std::path::Path,
        subdir: &str,
        structure: &DirectoryStructure,
    ) -> ResourceSummary {
        let dir_path = base_path.join(subdir);

        if !dir_path.exists() {
            return ResourceSummary {
                items: vec![],
                directory_exists: false,
            };
        }

        let items = match structure {
            DirectoryStructure::Flat { file_pattern } => {
                Self::list_files_matching(&dir_path, file_pattern)
            }
            DirectoryStructure::Nested {
                subdir_pattern,
                file_name,
            } => Self::list_subdirs_with_file(&dir_path, subdir_pattern, file_name),
        };

        ResourceSummary {
            items,
            directory_exists: true,
        }
    }

    fn list_files_matching(dir: &std::path::Path, pattern: &str) -> Vec<String> {
        std::fs::read_dir(dir)
            .ok()
            .map(|entries| {
                let mut items: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                    .filter(|e| Self::matches_pattern(e.file_name().to_str(), pattern))
                    .filter_map(|e| e.path().file_stem()?.to_str().map(String::from))
                    .collect();
                items.sort();
                items
            })
            .unwrap_or_default()
    }

    fn list_subdirs_with_file(
        dir: &std::path::Path,
        subdir_pattern: &str,
        file_name: &str,
    ) -> Vec<String> {
        std::fs::read_dir(dir)
            .ok()
            .map(|entries| {
                let mut items: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .filter(|e| Self::matches_pattern(e.file_name().to_str(), subdir_pattern))
                    .filter(|e| e.path().join(file_name).exists())
                    .filter_map(|e| e.file_name().to_str().map(String::from))
                    .collect();
                items.sort();
                items
            })
            .unwrap_or_default()
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.profiles_dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.profiles_dir.clone())
            .join("backups")
    }

    pub fn backup_current(&self, harness: &dyn HarnessConfig) -> Result<PathBuf> {
        let source_dir = harness.config_dir()?;

        if !source_dir.exists() {
            return Err(Error::NoConfigFound(format!(
                "No config found for {}",
                harness.id()
            )));
        }

        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = self.backups_dir().join(harness.id()).join(&timestamp);

        std::fs::create_dir_all(&backup_path)?;

        for entry in std::fs::read_dir(&source_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let dest = backup_path.join(entry.file_name());
                std::fs::copy(entry.path(), dest)?;
            }
        }

        Ok(backup_path)
    }

    fn save_to_profile(&self, harness: &dyn HarnessConfig, name: &ProfileName) -> Result<()> {
        let profile_path = self.profile_path(harness, name);
        if !profile_path.exists() {
            return Ok(());
        }

        let source_dir = harness.config_dir()?;
        if !source_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&profile_path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                std::fs::remove_file(entry.path())?;
            }
        }

        for entry in std::fs::read_dir(&source_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let dest = profile_path.join(entry.file_name());
                std::fs::copy(entry.path(), dest)?;
            }
        }

        Ok(())
    }

    pub fn switch_profile(
        &self,
        harness: &dyn HarnessConfig,
        name: &ProfileName,
    ) -> Result<PathBuf> {
        let profile_path = self.profile_path(harness, name);

        if !profile_path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        let harness_id = harness.id();
        if let Ok(config) = BridleConfig::load()
            && let Some(active_name) = config.active_profile_for(harness_id)
            && let Ok(active_profile) = ProfileName::new(active_name)
            && active_profile.as_str() != name.as_str()
        {
            let _ = self.save_to_profile(harness, &active_profile);
        }

        let target_dir = harness.config_dir()?;

        let temp_dir = target_dir.with_extension("bridle_tmp");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)?;
        }
        std::fs::create_dir_all(&temp_dir)?;

        for entry in std::fs::read_dir(&profile_path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let dest = temp_dir.join(entry.file_name());
                std::fs::copy(entry.path(), dest)?;
            }
        }

        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir)?;
        }
        std::fs::rename(&temp_dir, &target_dir)?;

        let mut config = BridleConfig::load().unwrap_or_default();
        config.set_active_profile(harness.id(), name.as_str());
        config.save()?;

        Ok(target_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct MockHarness {
        id: String,
        config_dir: PathBuf,
    }

    impl MockHarness {
        fn new(id: &str, config_dir: PathBuf) -> Self {
            Self {
                id: id.to_string(),
                config_dir,
            }
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

        fn parse_mcp_servers(&self, _content: &str) -> Result<Vec<(String, bool)>> {
            Ok(vec![])
        }
    }

    #[test]
    fn switch_profile_preserves_edits() {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        let live_config = temp.path().join("live_config");
        fs::create_dir_all(&live_config).unwrap();

        let harness = MockHarness::new("test-harness", live_config.clone());
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
    fn list_files_matching_finds_files_with_extension() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path();

        fs::write(dir.join("skill1.md"), "content").unwrap();
        fs::write(dir.join("skill2.md"), "content").unwrap();
        fs::write(dir.join("readme.txt"), "content").unwrap();
        fs::create_dir(dir.join("subdir")).unwrap();

        let result = ProfileManager::list_files_matching(dir, "*.md");

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

        let result = ProfileManager::list_subdirs_with_file(dir, "*", "index.md");

        assert_eq!(result, vec!["cmd1", "cmd2"]);
    }

    #[test]
    fn extract_resource_summary_handles_nonexistent_dir() {
        let temp = TempDir::new().unwrap();
        let structure = DirectoryStructure::Flat {
            file_pattern: "*.md".to_string(),
        };

        let result =
            ProfileManager::extract_resource_summary(temp.path(), "nonexistent", &structure);

        assert!(!result.directory_exists);
        assert!(result.items.is_empty());
    }
}
