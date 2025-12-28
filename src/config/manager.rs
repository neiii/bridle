//! Profile management.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::Local;
use get_harness::{Harness, InstallationStatus, McpServer, Scope};

use super::BridleConfig;
use super::profile_name::ProfileName;
use crate::error::{Error, Result};

/// Information about a profile for display purposes.
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    /// Profile name.
    pub name: String,
    /// Harness identifier.
    pub harness_id: String,
    /// Whether this is the currently active profile.
    pub is_active: bool,
    /// List of MCP server names configured in this profile.
    pub mcp_servers: Vec<String>,
    /// Path to the profile directory.
    pub path: PathBuf,
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

    pub fn harness_id(harness: &Harness) -> &'static str {
        match harness.kind() {
            get_harness::HarnessKind::ClaudeCode => "claude-code",
            get_harness::HarnessKind::OpenCode => "opencode",
            get_harness::HarnessKind::Goose => "goose",
            _ => "unknown",
        }
    }

    pub fn profile_path(&self, harness: &Harness, name: &ProfileName) -> PathBuf {
        self.profiles_dir
            .join(Self::harness_id(harness))
            .join(name.as_str())
    }

    pub fn profile_exists(&self, harness: &Harness, name: &ProfileName) -> bool {
        self.profile_path(harness, name).is_dir()
    }

    pub fn list_profiles(&self, harness: &Harness) -> Result<Vec<ProfileName>> {
        let harness_dir = self.profiles_dir.join(Self::harness_id(harness));

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

    pub fn create_profile(&self, harness: &Harness, name: &ProfileName) -> Result<PathBuf> {
        let path = self.profile_path(harness, name);

        if path.exists() {
            return Err(Error::ProfileExists(name.as_str().to_string()));
        }

        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    pub fn create_from_current(&self, harness: &Harness, name: &ProfileName) -> Result<PathBuf> {
        let profile_path = self.create_profile(harness, name)?;
        let source_dir = harness.config(&Scope::Global)?;

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
    pub fn create_from_current_if_missing(&self, harness: &Harness) -> Result<bool> {
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

    pub fn delete_profile(&self, harness: &Harness, name: &ProfileName) -> Result<()> {
        let path = self.profile_path(harness, name);

        if !path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        std::fs::remove_dir_all(&path)?;
        Ok(())
    }

    pub fn show_profile(&self, harness: &Harness, name: &ProfileName) -> Result<ProfileInfo> {
        let path = self.profile_path(harness, name);

        if !path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        let harness_id = Self::harness_id(harness).to_string();
        let is_active = BridleConfig::load()
            .map(|c| c.active_profile_for(&harness_id) == Some(name.as_str()))
            .unwrap_or(false);

        let mcp_servers = self.extract_mcp_servers(harness, &path)?;

        Ok(ProfileInfo {
            name: name.as_str().to_string(),
            harness_id,
            is_active,
            mcp_servers,
            path,
        })
    }

    fn extract_mcp_servers(
        &self,
        harness: &Harness,
        profile_path: &std::path::Path,
    ) -> Result<Vec<String>> {
        let mcp_resource = match harness.mcp(&Scope::Global)? {
            Some(r) => r,
            None => return Ok(Vec::new()),
        };
        let mcp_filename = mcp_resource
            .file
            .file_name()
            .and_then(|n: &std::ffi::OsStr| n.to_str())
            .unwrap_or("config.json");

        let profile_mcp_path = profile_path.join(mcp_filename);

        if !profile_mcp_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&profile_mcp_path)?;
        let servers = self.parse_mcp_server_names(harness, &content)?;
        Ok(servers)
    }

    fn parse_mcp_server_names(&self, harness: &Harness, content: &str) -> Result<Vec<String>> {
        let parsed: serde_json::Value = serde_json::from_str(content)?;
        let servers: HashMap<String, McpServer> = harness.parse_mcp_config(&parsed)?;
        let mut names: Vec<String> = servers.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.profiles_dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.profiles_dir.clone())
            .join("backups")
    }

    pub fn backup_current(&self, harness: &Harness) -> Result<PathBuf> {
        let source_dir = harness.config(&Scope::Global)?;

        if !source_dir.exists() {
            return Err(Error::NoConfigFound(format!(
                "No config found for {}",
                Self::harness_id(harness)
            )));
        }

        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = self
            .backups_dir()
            .join(Self::harness_id(harness))
            .join(&timestamp);

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

    pub fn switch_profile(&self, harness: &Harness, name: &ProfileName) -> Result<PathBuf> {
        let profile_path = self.profile_path(harness, name);

        if !profile_path.exists() {
            return Err(Error::ProfileNotFound(name.as_str().to_string()));
        }

        let target_dir = harness.config(&Scope::Global)?;

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

        let harness_id = Self::harness_id(harness);
        let mut config = BridleConfig::load().unwrap_or_default();
        config.set_active_profile(harness_id, name.as_str());
        config.save()?;

        Ok(target_dir)
    }
}
