//! Bridle's own configuration file handling.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// User preference for TUI view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewPreference {
    /// Classic table-based view.
    #[default]
    Dashboard,
    /// Legacy list view.
    Legacy,
    /// Card-based view (requires tui-cards feature).
    #[cfg(feature = "tui-cards")]
    Cards,
}

/// TUI-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TuiConfig {
    /// Preferred view mode.
    #[serde(default)]
    pub view: ViewPreference,
}

/// Bridle's configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BridleConfig {
    /// Active profile per harness (harness_id -> profile_name).
    #[serde(default)]
    pub active: HashMap<String, String>,

    /// Whether to create `BRIDLE_PROFILE_<name>` marker files in harness config directories.
    /// Disabled by default (opt-in).
    #[serde(default)]
    pub profile_marker: bool,

    /// Legacy field for migration (ignored on save).
    #[serde(skip_serializing, default)]
    active_profile: Option<String>,

    /// Preferred editor for editing profiles.
    /// Falls back to $EDITOR env var, then "vi".
    #[serde(default)]
    pub editor: Option<String>,

    /// TUI-specific settings.
    #[serde(default)]
    pub tui: TuiConfig,

    /// Default harness to show when TUI opens.
    #[serde(default)]
    pub default_harness: Option<String>,
}

impl BridleConfig {
    pub fn editor(&self) -> String {
        self.editor
            .clone()
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "vi".to_string())
    }
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
        harness_locate::platform::config_dir()
            .map(|d| d.join("bridle").join("config.toml"))
            .map_err(|e| crate::error::Error::NoConfigFound(e.to_string()))
    }

    /// Get the configuration directory path.
    ///
    /// Respects the `BRIDLE_CONFIG_DIR` environment variable for testing.
    pub fn config_dir() -> crate::error::Result<PathBuf> {
        if let Ok(dir) = std::env::var("BRIDLE_CONFIG_DIR") {
            return Ok(PathBuf::from(dir));
        }
        harness_locate::platform::config_dir()
            .map(|d| d.join("bridle"))
            .map_err(|e| crate::error::Error::NoConfigFound(e.to_string()))
    }

    /// Get the profiles directory path.
    pub fn profiles_dir() -> crate::error::Result<PathBuf> {
        Self::config_dir().map(|d| d.join("profiles"))
    }

    /// Save configuration to the default location.
    pub fn save(&self) -> crate::error::Result<()> {
        let path = Self::config_path()?;
        let content =
            toml::to_string_pretty(self).map_err(|e| crate::error::Error::Config(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Get the active profile for a harness.
    pub fn active_profile_for(&self, harness_id: &str) -> Option<&str> {
        self.active.get(harness_id).map(|s| s.as_str())
    }

    /// Set the active profile for a harness.
    pub fn set_active_profile(&mut self, harness_id: &str, profile: &str) {
        self.active
            .insert(harness_id.to_string(), profile.to_string());
    }

    /// Clear the active profile for a harness.
    pub fn clear_active_profile(&mut self, harness_id: &str) {
        self.active.remove(harness_id);
    }

    pub fn profile_marker_enabled(&self) -> bool {
        self.profile_marker
    }

    pub fn set_profile_marker(&mut self, enabled: bool) {
        self.profile_marker = enabled;
    }

    pub fn default_harness(&self) -> Option<&str> {
        self.default_harness.as_deref()
    }

    pub fn set_default_harness(&mut self, harness_id: Option<&str>) {
        self.default_harness = harness_id.map(String::from);
    }
}
