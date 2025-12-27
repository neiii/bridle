//! Bridle's own configuration file handling.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Bridle's configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridleConfig {
    /// Active profile name.
    pub active_profile: Option<String>,

    /// Path to profiles directory.
    pub profiles_dir: Option<PathBuf>,
}

impl BridleConfig {
    /// Load configuration from the default location.
    pub fn load() -> crate::error::Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Self = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Get the default configuration file path.
    pub fn config_path() -> crate::error::Result<PathBuf> {
        dirs::config_dir()
            .map(|d| d.join("bridle").join("config.toml"))
            .ok_or_else(|| crate::error::Error::NoConfigFound("config directory".into()))
    }

    /// Get the configuration directory path.
    pub fn config_dir() -> crate::error::Result<PathBuf> {
        dirs::config_dir()
            .map(|d| d.join("bridle"))
            .ok_or_else(|| crate::error::Error::NoConfigFound("config directory".into()))
    }

    /// Save configuration to the default location.
    pub fn save(&self) -> crate::error::Result<()> {
        let path = Self::config_path()?;
        let content =
            toml::to_string_pretty(self).map_err(|e| crate::error::Error::Config(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
