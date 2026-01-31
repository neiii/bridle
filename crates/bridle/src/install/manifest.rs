use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::types::{ComponentType, SourceInfo};

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("Failed to read manifest: {0}")]
    Read(#[source] std::io::Error),

    #[error("Failed to write manifest: {0}")]
    Write(#[source] std::io::Error),

    #[error("Failed to parse manifest: {0}")]
    Parse(#[source] serde_json::Error),

    #[error("Failed to serialize manifest: {0}")]
    Serialize(#[source] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub component_type: ComponentType,
    pub name: String,
    pub source: SourceInfo,
    pub installed_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct InstallManifest {
    pub entries: Vec<ManifestEntry>,
}

impl InstallManifest {
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path).map_err(ManifestError::Read)?;
        serde_json::from_str(&content).map_err(ManifestError::Parse)
    }

    pub fn save(&self, path: &Path) -> Result<(), ManifestError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ManifestError::Write)?;
        }

        let content = serde_json::to_string_pretty(self).map_err(ManifestError::Serialize)?;
        fs::write(path, content).map_err(ManifestError::Write)
    }

    pub fn add_entry(&mut self, entry: ManifestEntry) {
        self.remove_component(entry.component_type, &entry.name);
        self.entries.push(entry);
    }

    pub fn remove_component(&mut self, component_type: ComponentType, name: &str) {
        self.entries
            .retain(|e| !(e.component_type as u8 == component_type as u8 && e.name == name));
    }

    pub fn find_component(
        &self,
        component_type: ComponentType,
        name: &str,
    ) -> Option<&ManifestEntry> {
        self.entries
            .iter()
            .find(|e| e.component_type as u8 == component_type as u8 && e.name == name)
    }
}

pub fn manifest_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join(".bridle-manifest.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_manifest_load_empty() {
        let temp = TempDir::new().unwrap();
        let path = manifest_path(temp.path());

        let manifest = InstallManifest::load(&path).unwrap();
        assert!(manifest.entries.is_empty());
    }

    #[test]
    fn test_manifest_save_load() {
        let temp = TempDir::new().unwrap();
        let path = manifest_path(temp.path());

        let mut manifest = InstallManifest::default();
        manifest.add_entry(ManifestEntry {
            component_type: ComponentType::Skill,
            name: "test-skill".to_string(),
            source: SourceInfo {
                owner: "test".to_string(),
                repo: "repo".to_string(),
                git_ref: Some("main".to_string()),
            },
            installed_at: "2025-01-02T12:00:00Z".to_string(),
        });

        manifest.save(&path).unwrap();

        let loaded = InstallManifest::load(&path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].name, "test-skill");
    }

    #[test]
    fn test_manifest_add_entry_replaces_existing() {
        let mut manifest = InstallManifest::default();

        manifest.add_entry(ManifestEntry {
            component_type: ComponentType::Skill,
            name: "skill".to_string(),
            source: SourceInfo {
                owner: "old".to_string(),
                repo: "repo".to_string(),
                git_ref: None,
            },
            installed_at: "2025-01-01T00:00:00Z".to_string(),
        });

        manifest.add_entry(ManifestEntry {
            component_type: ComponentType::Skill,
            name: "skill".to_string(),
            source: SourceInfo {
                owner: "new".to_string(),
                repo: "repo".to_string(),
                git_ref: None,
            },
            installed_at: "2025-01-02T00:00:00Z".to_string(),
        });

        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].source.owner, "new");
    }

    #[test]
    fn test_manifest_remove_component() {
        let mut manifest = InstallManifest::default();

        manifest.add_entry(ManifestEntry {
            component_type: ComponentType::Skill,
            name: "skill1".to_string(),
            source: SourceInfo {
                owner: "test".to_string(),
                repo: "repo".to_string(),
                git_ref: None,
            },
            installed_at: "2025-01-02T00:00:00Z".to_string(),
        });

        manifest.add_entry(ManifestEntry {
            component_type: ComponentType::Agent,
            name: "agent1".to_string(),
            source: SourceInfo {
                owner: "test".to_string(),
                repo: "repo".to_string(),
                git_ref: None,
            },
            installed_at: "2025-01-02T00:00:00Z".to_string(),
        });

        manifest.remove_component(ComponentType::Skill, "skill1");
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].name, "agent1");
    }

    #[test]
    fn test_manifest_find_component() {
        let mut manifest = InstallManifest::default();

        manifest.add_entry(ManifestEntry {
            component_type: ComponentType::Skill,
            name: "skill1".to_string(),
            source: SourceInfo {
                owner: "test".to_string(),
                repo: "repo".to_string(),
                git_ref: None,
            },
            installed_at: "2025-01-02T00:00:00Z".to_string(),
        });

        let found = manifest.find_component(ComponentType::Skill, "skill1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "skill1");

        let not_found = manifest.find_component(ComponentType::Agent, "skill1");
        assert!(not_found.is_none());
    }
}
