use std::path::PathBuf;

use chrono::Local;
use harness_locate::Harness;

use super::ProfileManager;
use super::files;
use crate::config::BridleConfig;
use crate::config::profile_name::ProfileName;
use crate::error::{Error, Result};
use crate::harness::HarnessConfig;

impl ProfileManager {
    pub fn backups_dir(&self) -> PathBuf {
        self.profiles_dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.profiles_dir.clone())
            .join("backups")
    }

    pub fn backup_current(&self, harness: &dyn HarnessConfig) -> Result<PathBuf> {
        let source_dir = harness.config_dir()?;
        let has_config_dir = source_dir.exists();
        let has_mcp = harness
            .mcp_config_path()
            .map(|p| p.exists())
            .unwrap_or(false);

        if !has_config_dir && !has_mcp {
            return Err(Error::NoConfigFound(format!(
                "No config found for {}",
                harness.id()
            )));
        }

        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let backup_path = self.backups_dir().join(harness.id()).join(&timestamp);

        std::fs::create_dir_all(&backup_path)?;
        files::copy_config_files(harness, true, &backup_path)?;

        Ok(backup_path)
    }

    pub fn save_to_profile(
        &self,
        harness: &dyn HarnessConfig,
        harness_for_resources: Option<&Harness>,
        name: &ProfileName,
    ) -> Result<()> {
        let profile_path = self.profile_path(harness, name);
        if !profile_path.exists() {
            return Ok(());
        }

        let source_dir = harness.config_dir()?;
        let has_config = source_dir.exists()
            || harness
                .mcp_config_path()
                .map(|p| p.exists())
                .unwrap_or(false);
        if !has_config {
            return Ok(());
        }

        for entry in std::fs::read_dir(&profile_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::remove_file(&path)?;
            } else if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
            }
        }

        files::copy_all_contents(&source_dir, &profile_path)?;
        if let Some(mcp_path) = harness.mcp_config_path()
            && mcp_path.exists()
            && mcp_path.is_file()
            && let Some(filename) = mcp_path.file_name()
        {
            let dest = profile_path.join(filename);
            std::fs::copy(&mcp_path, dest)?;
        }
        let _ = harness_for_resources;
        Ok(())
    }

    pub fn switch_profile(
        &self,
        harness: &dyn HarnessConfig,
        name: &ProfileName,
    ) -> Result<PathBuf> {
        self.switch_profile_with_resources(harness, None, name)
    }

    pub fn switch_profile_with_resources(
        &self,
        harness: &dyn HarnessConfig,
        harness_for_resources: Option<&Harness>,
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
            let _ = self.save_to_profile(harness, harness_for_resources, &active_profile);
        }

        let target_dir = harness.config_dir()?;

        if !target_dir.exists() {
            std::fs::create_dir_all(&target_dir)?;
        }

        files::copy_all_contents(&profile_path, &target_dir)?;

        if let Some(mcp_path) = harness.mcp_config_path()
            && let Some(filename) = mcp_path.file_name()
        {
            let mcp_in_profile = profile_path.join(filename);
            if mcp_in_profile.exists() {
                std::fs::copy(&mcp_in_profile, &mcp_path)?;
            }
        }

        let _ = harness_for_resources;

        let mut config = BridleConfig::load().unwrap_or_default();
        config.set_active_profile(harness.id(), name.as_str());
        config.save()?;

        Self::delete_marker_files(&target_dir)?;
        if config.profile_marker_enabled() {
            Self::create_marker_file(&target_dir, name.as_str())?;
        }

        Ok(target_dir)
    }

    pub fn update_marker_file(
        harness: &dyn HarnessConfig,
        profile_name: Option<&str>,
        enabled: bool,
    ) -> Result<()> {
        let config_dir = harness.config_dir()?;
        Self::delete_marker_files(&config_dir)?;
        if let (true, Some(name)) = (enabled, profile_name) {
            Self::create_marker_file(&config_dir, name)?;
        }
        Ok(())
    }
}
