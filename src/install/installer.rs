//! Skill installation executor.

use std::fs;
use std::path::PathBuf;

use thiserror::Error;

use harness_locate::{Harness, HarnessKind, Scope};

use super::manifest::{InstallManifest, ManifestEntry, manifest_path};
use super::types::{
    AgentInfo, CommandInfo, ComponentType, InstallFailure, InstallOptions, InstallReport,
    InstallSkip, InstallSuccess, InstallTarget, SkillInfo, SkipReason, SourceInfo,
};
use crate::config::BridleConfig;
use crate::harness::HarnessConfig;

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("Failed to create directory: {0}")]
    CreateDir(#[source] std::io::Error),

    #[error("Failed to write file: {0}")]
    WriteFile(#[source] std::io::Error),

    #[error("Profile directory not found for {harness}/{profile}")]
    ProfileNotFound { harness: String, profile: String },

    #[error("Harness not found: {0}")]
    HarnessNotFound(String),

    #[error("Invalid component name: {0}")]
    InvalidComponentName(String),
}

fn validate_component_name(name: &str) -> Result<(), InstallError> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || name.contains('\0')
    {
        return Err(InstallError::InvalidComponentName(name.to_string()));
    }
    Ok(())
}

fn parse_harness_kind(id: &str) -> Option<HarnessKind> {
    match id {
        "claude-code" | "claude" | "cc" => Some(HarnessKind::ClaudeCode),
        "opencode" | "oc" => Some(HarnessKind::OpenCode),
        "goose" => Some(HarnessKind::Goose),
        "amp-code" | "amp" | "ampcode" => Some(HarnessKind::AmpCode),
        _ => None,
    }
}

fn get_agents_dir_name(harness_id: &str) -> &'static str {
    match harness_id {
        "opencode" | "oc" => "agent",
        _ => "agents",
    }
}

fn get_commands_dir_name(harness_id: &str) -> &'static str {
    match harness_id {
        "opencode" | "oc" => "command",
        _ => "commands",
    }
}

pub fn install_skill(
    skill: &SkillInfo,
    target: &InstallTarget,
    options: &InstallOptions,
) -> InstallResult {
    let profiles_dir = BridleConfig::profiles_dir().map_err(|_| InstallError::ProfileNotFound {
        harness: target.harness.clone(),
        profile: target.profile.as_str().to_string(),
    })?;

    install_skill_to_dir(skill, target, options, &profiles_dir)
}

fn install_skill_to_dir(
    skill: &SkillInfo,
    target: &InstallTarget,
    options: &InstallOptions,
    profiles_dir: &std::path::Path,
) -> InstallResult {
    install_skill_to_dir_with_source(skill, target, options, profiles_dir, None)
}

fn install_skill_to_dir_with_source(
    skill: &SkillInfo,
    target: &InstallTarget,
    options: &InstallOptions,
    profiles_dir: &std::path::Path,
    source: Option<&SourceInfo>,
) -> InstallResult {
    validate_component_name(&skill.name)?;

    let profile_dir = profiles_dir
        .join(&target.harness)
        .join(target.profile.as_str());

    if !profile_dir.exists() {
        return Err(InstallError::ProfileNotFound {
            harness: target.harness.clone(),
            profile: target.profile.as_str().to_string(),
        });
    }

    let skill_dir = profile_dir.join("skills").join(&skill.name);
    let skill_path = skill_dir.join("SKILL.md");

    if skill_path.exists() && !options.force {
        return Ok(InstallOutcome::Skipped(InstallSkip {
            skill: skill.name.clone(),
            target: target.clone(),
            reason: SkipReason::AlreadyExists,
        }));
    }

    fs::create_dir_all(&skill_dir).map_err(InstallError::CreateDir)?;
    fs::write(&skill_path, &skill.content).map_err(InstallError::WriteFile)?;

    if let Some(source_info) = source {
        update_manifest(&profile_dir, ComponentType::Skill, &skill.name, source_info);
    }

    let harness_path = write_to_harness_if_active(target, skill)?;

    Ok(InstallOutcome::Installed(InstallSuccess {
        skill: skill.name.clone(),
        target: target.clone(),
        profile_path: skill_path,
        harness_path,
    }))
}

fn write_to_harness_if_active(
    target: &InstallTarget,
    skill: &SkillInfo,
) -> Result<Option<PathBuf>, InstallError> {
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
        .ok_or_else(|| InstallError::HarnessNotFound(target.harness.clone()))?;
    let harness =
        Harness::locate(kind).map_err(|_| InstallError::HarnessNotFound(target.harness.clone()))?;

    let skills_dir = harness
        .skills(&Scope::Global)
        .ok()
        .flatten()
        .map(|r| r.path)
        .unwrap_or_else(|| {
            harness
                .config_dir()
                .map(|d| d.join("skills"))
                .unwrap_or_default()
        });
    let harness_skill_dir = skills_dir.join(&skill.name);
    let harness_skill_path = harness_skill_dir.join("SKILL.md");

    fs::create_dir_all(&harness_skill_dir).map_err(InstallError::CreateDir)?;
    fs::write(&harness_skill_path, &skill.content).map_err(InstallError::WriteFile)?;

    Ok(Some(harness_skill_path))
}

fn update_manifest(
    profile_dir: &std::path::Path,
    component_type: ComponentType,
    name: &str,
    source: &SourceInfo,
) {
    let manifest_file = manifest_path(profile_dir);
    let mut manifest = InstallManifest::load(&manifest_file).unwrap_or_default();

    manifest.add_entry(ManifestEntry {
        component_type,
        name: name.to_string(),
        source: source.clone(),
        installed_at: chrono::Utc::now().to_rfc3339(),
    });

    let _ = manifest.save(&manifest_file);
}

pub enum InstallOutcome {
    Installed(InstallSuccess),
    Skipped(InstallSkip),
}

pub type InstallResult = Result<InstallOutcome, InstallError>;

pub fn install_agent(
    agent: &AgentInfo,
    target: &InstallTarget,
    options: &InstallOptions,
) -> InstallResult {
    install_agent_with_source(agent, target, options, None)
}

fn install_agent_with_source(
    agent: &AgentInfo,
    target: &InstallTarget,
    options: &InstallOptions,
    source: Option<&SourceInfo>,
) -> InstallResult {
    let profiles_dir = BridleConfig::profiles_dir().map_err(|_| InstallError::ProfileNotFound {
        harness: target.harness.clone(),
        profile: target.profile.as_str().to_string(),
    })?;

    validate_component_name(&agent.name)?;

    let profile_dir = profiles_dir
        .join(&target.harness)
        .join(target.profile.as_str());

    if !profile_dir.exists() {
        return Err(InstallError::ProfileNotFound {
            harness: target.harness.clone(),
            profile: target.profile.as_str().to_string(),
        });
    }

    let agents_dir = profile_dir.join(get_agents_dir_name(&target.harness));
    let agent_path = agents_dir.join(format!("{}.md", &agent.name));

    if agent_path.exists() && !options.force {
        return Ok(InstallOutcome::Skipped(InstallSkip {
            skill: agent.name.clone(),
            target: target.clone(),
            reason: SkipReason::AlreadyExists,
        }));
    }

    fs::create_dir_all(&agents_dir).map_err(InstallError::CreateDir)?;
    fs::write(&agent_path, &agent.content).map_err(InstallError::WriteFile)?;

    if let Some(source_info) = source {
        update_manifest(&profile_dir, ComponentType::Agent, &agent.name, source_info);
    }

    Ok(InstallOutcome::Installed(InstallSuccess {
        skill: agent.name.clone(),
        target: target.clone(),
        profile_path: agent_path,
        harness_path: None,
    }))
}

pub fn install_command(
    command: &CommandInfo,
    target: &InstallTarget,
    options: &InstallOptions,
) -> InstallResult {
    install_command_with_source(command, target, options, None)
}

fn install_command_with_source(
    command: &CommandInfo,
    target: &InstallTarget,
    options: &InstallOptions,
    source: Option<&SourceInfo>,
) -> InstallResult {
    let profiles_dir = BridleConfig::profiles_dir().map_err(|_| InstallError::ProfileNotFound {
        harness: target.harness.clone(),
        profile: target.profile.as_str().to_string(),
    })?;

    validate_component_name(&command.name)?;

    let profile_dir = profiles_dir
        .join(&target.harness)
        .join(target.profile.as_str());

    if !profile_dir.exists() {
        return Err(InstallError::ProfileNotFound {
            harness: target.harness.clone(),
            profile: target.profile.as_str().to_string(),
        });
    }

    let commands_dir = profile_dir.join(get_commands_dir_name(&target.harness));
    let command_path = commands_dir.join(format!("{}.md", &command.name));

    if command_path.exists() && !options.force {
        return Ok(InstallOutcome::Skipped(InstallSkip {
            skill: command.name.clone(),
            target: target.clone(),
            reason: SkipReason::AlreadyExists,
        }));
    }

    fs::create_dir_all(&commands_dir).map_err(InstallError::CreateDir)?;
    fs::write(&command_path, &command.content).map_err(InstallError::WriteFile)?;

    if let Some(source_info) = source {
        update_manifest(
            &profile_dir,
            ComponentType::Command,
            &command.name,
            source_info,
        );
    }

    Ok(InstallOutcome::Installed(InstallSuccess {
        skill: command.name.clone(),
        target: target.clone(),
        profile_path: command_path,
        harness_path: None,
    }))
}

pub fn install_skills(
    skills: &[SkillInfo],
    target: &InstallTarget,
    options: &InstallOptions,
) -> InstallReport {
    let mut installed = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();

    for skill in skills {
        match install_skill(skill, target, options) {
            Ok(InstallOutcome::Installed(success)) => installed.push(success),
            Ok(InstallOutcome::Skipped(skip)) => skipped.push(skip),
            Err(e) => errors.push(InstallFailure {
                skill: skill.name.clone(),
                target: target.clone(),
                error: e.to_string(),
            }),
        }
    }

    InstallReport {
        installed,
        skipped,
        errors,
    }
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
    fn install_creates_skill_directory() {
        let (_temp, target, profiles_dir) = setup_test_env();

        let skill = SkillInfo {
            name: "my-skill".to_string(),
            description: Some("A test skill".to_string()),
            path: "skills/my-skill/SKILL.md".to_string(),
            content: "# My Skill\n\nContent here".to_string(),
        };

        let result =
            install_skill_to_dir(&skill, &target, &InstallOptions::default(), &profiles_dir);
        assert!(result.is_ok());

        if let Ok(InstallOutcome::Installed(success)) = result {
            assert!(success.profile_path.exists());
            let content = fs::read_to_string(&success.profile_path).unwrap();
            assert_eq!(content, "# My Skill\n\nContent here");
        }
    }

    #[test]
    fn install_skips_existing_without_force() {
        let (temp, target, profiles_dir) = setup_test_env();

        let skill_dir = temp.path().join("profiles/opencode/test/skills/existing");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "existing").unwrap();

        let skill = SkillInfo {
            name: "existing".to_string(),
            description: None,
            path: "skills/existing/SKILL.md".to_string(),
            content: "new content".to_string(),
        };

        let result =
            install_skill_to_dir(&skill, &target, &InstallOptions::default(), &profiles_dir);
        assert!(matches!(result, Ok(InstallOutcome::Skipped(_))));
    }

    #[test]
    fn install_overwrites_with_force() {
        let (temp, target, profiles_dir) = setup_test_env();

        let skill_dir = temp.path().join("profiles/opencode/test/skills/existing");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "old content").unwrap();

        let skill = SkillInfo {
            name: "existing".to_string(),
            description: None,
            path: "skills/existing/SKILL.md".to_string(),
            content: "new content".to_string(),
        };

        let result = install_skill_to_dir(
            &skill,
            &target,
            &InstallOptions { force: true },
            &profiles_dir,
        );
        assert!(matches!(result, Ok(InstallOutcome::Installed(_))));

        let content = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn install_rejects_invalid_skill_names() {
        let (_temp, target, profiles_dir) = setup_test_env();

        let invalid_names = ["", "../escape", "path/traversal", ".", "..", "null\0char"];
        for name in invalid_names {
            let skill = SkillInfo {
                name: name.to_string(),
                description: None,
                path: String::new(),
                content: "content".to_string(),
            };
            let result =
                install_skill_to_dir(&skill, &target, &InstallOptions::default(), &profiles_dir);
            assert!(
                matches!(result, Err(InstallError::InvalidComponentName(_))),
                "Expected InvalidComponentName for '{name}'"
            );
        }
    }

    #[test]
    fn install_returns_error_for_missing_profile() {
        let temp = TempDir::new().unwrap();
        let profiles_dir = temp.path().join("profiles");
        fs::create_dir_all(&profiles_dir).unwrap();

        let target = InstallTarget {
            harness: "opencode".to_string(),
            profile: ProfileName::new("nonexistent").unwrap(),
        };

        let skill = SkillInfo {
            name: "skill".to_string(),
            description: None,
            path: "skills/skill/SKILL.md".to_string(),
            content: "content".to_string(),
        };

        let result =
            install_skill_to_dir(&skill, &target, &InstallOptions::default(), &profiles_dir);
        assert!(matches!(result, Err(InstallError::ProfileNotFound { .. })));
    }
}
