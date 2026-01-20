//! Component uninstallation executor.

use std::fs;
use std::path::PathBuf;

use thiserror::Error;

use harness_locate::{Harness, HarnessKind, Scope};

use super::manifest::{InstallManifest, manifest_path};
use super::types::{
    ComponentType, InstallTarget, UninstallFailure, UninstallReport, UninstallSuccess,
};
use crate::config::BridleConfig;
use crate::harness::HarnessConfig;

#[derive(Debug, Error)]
pub enum UninstallError {
    #[error("Failed to remove directory: {0}")]
    RemoveDir(#[source] std::io::Error),

    #[error("Profile directory not found for {harness}/{profile}")]
    ProfileNotFound { harness: String, profile: String },

    #[error("Component not found: {0}")]
    ComponentNotFound(String),

    #[error("Harness not found: {0}")]
    HarnessNotFound(String),
}

fn parse_harness_kind(id: &str) -> Option<HarnessKind> {
    match id {
        "claude-code" | "claude" | "cc" => Some(HarnessKind::ClaudeCode),
        "opencode" | "oc" => Some(HarnessKind::OpenCode),
        "goose" => Some(HarnessKind::Goose),
        "amp-code" | "amp" | "ampcode" => Some(HarnessKind::AmpCode),
        "copilot-cli" | "copilot" | "ghcp" => Some(HarnessKind::CopilotCli),
        "crush" => Some(HarnessKind::Crush),
        _ => None,
    }
}

pub fn uninstall_component(
    component_name: &str,
    component_type: ComponentType,
    target: &InstallTarget,
) -> Result<UninstallSuccess, UninstallError> {
    let profiles_dir =
        BridleConfig::profiles_dir().map_err(|_| UninstallError::ProfileNotFound {
            harness: target.harness.clone(),
            profile: target.profile.as_str().to_string(),
        })?;

    uninstall_component_from_dir(component_name, component_type, target, &profiles_dir)
}

fn uninstall_component_from_dir(
    component_name: &str,
    component_type: ComponentType,
    target: &InstallTarget,
    profiles_dir: &std::path::Path,
) -> Result<UninstallSuccess, UninstallError> {
    let profile_dir = profiles_dir
        .join(&target.harness)
        .join(target.profile.as_str());

    if !profile_dir.exists() {
        return Err(UninstallError::ProfileNotFound {
            harness: target.harness.clone(),
            profile: target.profile.as_str().to_string(),
        });
    }

    let component_dir = profile_dir
        .join(component_type.dir_name())
        .join(component_name);

    if !component_dir.exists() {
        return Err(UninstallError::ComponentNotFound(
            component_name.to_string(),
        ));
    }

    fs::remove_dir_all(&component_dir).map_err(UninstallError::RemoveDir)?;

    let manifest_file = manifest_path(&profile_dir);
    if let Ok(mut manifest) = InstallManifest::load(&manifest_file) {
        manifest.remove_component(component_type, component_name);
        let _ = manifest.save(&manifest_file);
    }

    let harness_path = remove_from_harness_if_active(target, component_name, component_type)?;

    Ok(UninstallSuccess {
        component: component_name.to_string(),
        component_type: format!("{:?}", component_type).to_lowercase(),
        target: target.clone(),
        profile_path: component_dir,
        harness_path,
    })
}

fn remove_from_harness_if_active(
    target: &InstallTarget,
    component_name: &str,
    component_type: ComponentType,
) -> Result<Option<PathBuf>, UninstallError> {
    let config = BridleConfig::load().ok();
    let is_active = config
        .as_ref()
        .and_then(|c| c.active_profile_for(&target.harness))
        .map(|active| active == target.profile.as_str())
        .unwrap_or(false);

    if !is_active {
        return Ok(None);
    }

    let kind = parse_harness_kind(&target.harness)
        .ok_or_else(|| UninstallError::HarnessNotFound(target.harness.clone()))?;
    let harness = Harness::locate(kind)
        .map_err(|_| UninstallError::HarnessNotFound(target.harness.clone()))?;

    let component_dir_result = match component_type {
        ComponentType::Skill => harness.skills(&Scope::Global),
        ComponentType::Agent => harness.agents(&Scope::Global),
        ComponentType::Command => harness.commands(&Scope::Global),
    };

    let harness_component_dir = component_dir_result
        .ok()
        .flatten()
        .map(|r| r.path.join(component_name))
        .unwrap_or_else(|| {
            harness
                .config_dir()
                .map(|d| d.join(component_type.dir_name()).join(component_name))
                .unwrap_or_default()
        });

    if harness_component_dir.exists() {
        fs::remove_dir_all(&harness_component_dir).map_err(UninstallError::RemoveDir)?;
        Ok(Some(harness_component_dir))
    } else {
        Ok(None)
    }
}

pub fn uninstall_components(
    components: &[(String, ComponentType)],
    target: &InstallTarget,
) -> UninstallReport {
    let mut removed = Vec::new();
    let mut errors = Vec::new();

    for (name, comp_type) in components {
        match uninstall_component(name, *comp_type, target) {
            Ok(success) => removed.push(success),
            Err(e) => errors.push(UninstallFailure {
                component: name.clone(),
                component_type: format!("{:?}", comp_type).to_lowercase(),
                target: target.clone(),
                error: e.to_string(),
            }),
        }
    }

    UninstallReport { removed, errors }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProfileName;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, InstallTarget, PathBuf) {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        let profile_dir = profiles_dir.join("opencode").join("test");
        fs::create_dir_all(&profile_dir).unwrap();

        let target = InstallTarget {
            harness: "opencode".to_string(),
            profile: ProfileName::new("test").unwrap(),
        };

        (temp, target, profiles_dir)
    }

    #[test]
    fn uninstall_removes_component_directory() {
        let (temp, target, profiles_dir) = setup_test_env();

        let skill_dir = temp.path().join("profiles/opencode/test/skills/test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "content").unwrap();

        assert!(skill_dir.exists());

        let result = uninstall_component_from_dir(
            "test-skill",
            ComponentType::Skill,
            &target,
            &profiles_dir,
        );
        assert!(result.is_ok());
        assert!(!skill_dir.exists());
    }

    #[test]
    fn uninstall_returns_error_for_missing_component() {
        let (_temp, target, profiles_dir) = setup_test_env();

        let result = uninstall_component_from_dir(
            "nonexistent",
            ComponentType::Skill,
            &target,
            &profiles_dir,
        );
        assert!(matches!(result, Err(UninstallError::ComponentNotFound(_))));
    }

    #[test]
    fn uninstall_returns_error_for_missing_profile() {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();

        let target = InstallTarget {
            harness: "opencode".to_string(),
            profile: ProfileName::new("nonexistent").unwrap(),
        };

        let result = uninstall_component_from_dir(
            "test-skill",
            ComponentType::Skill,
            &target,
            &profiles_dir,
        );
        assert!(matches!(
            result,
            Err(UninstallError::ProfileNotFound { .. })
        ));
    }
}
