//! Skill installation executor.

use std::fs;
use std::path::{Path, PathBuf};

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
        "copilot-cli" | "copilot" | "ghcp" => Some(HarnessKind::CopilotCli),
        "crush" => Some(HarnessKind::Crush),
        "droid" | "factory" => Some(HarnessKind::Droid),
        _ => None,
    }
}

pub fn sanitize_name_for_opencode(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn transform_skill_for_opencode(content: &str, sanitized_dir_name: &str) -> String {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return format!(
            "---\nname: {}\ndescription: Skill installed by Bridle\n---\n{}",
            sanitized_dir_name, content
        );
    }

    let frontmatter = parts[1];
    let body = parts[2];

    let mut new_lines: Vec<String> = Vec::new();
    let mut found_name = false;
    let mut found_description = false;

    for line in frontmatter.lines() {
        let trimmed = line.trim_start();

        if trimmed.starts_with("name:") {
            new_lines.push(format!("name: {}", sanitized_dir_name));
            found_name = true;
            continue;
        }

        if trimmed.starts_with("description:") {
            found_description = true;
        }

        new_lines.push(line.to_string());
    }

    if !found_name {
        new_lines.insert(0, format!("name: {}", sanitized_dir_name));
    }

    if !found_description {
        let insert_pos = new_lines
            .iter()
            .position(|l| l.trim_start().starts_with("name:"))
            .map(|p| p + 1)
            .unwrap_or(0);
        new_lines.insert(
            insert_pos,
            "description: Skill installed by Bridle".to_string(),
        );
    }

    let new_frontmatter = new_lines.join("\n");
    format!("---\n{}\n---{}", new_frontmatter.trim(), body)
}

fn color_name_to_hex(name: &str) -> Option<&'static str> {
    match name.to_lowercase().trim() {
        "red" => Some("#FF0000"),
        "green" => Some("#00FF00"),
        "blue" => Some("#0000FF"),
        "yellow" => Some("#FFFF00"),
        "orange" => Some("#FFA500"),
        "purple" => Some("#800080"),
        "cyan" => Some("#00FFFF"),
        "magenta" => Some("#FF00FF"),
        "white" => Some("#FFFFFF"),
        "black" => Some("#000000"),
        "gray" | "grey" => Some("#808080"),
        "pink" => Some("#FFC0CB"),
        "brown" => Some("#A52A2A"),
        "lime" => Some("#00FF00"),
        "navy" => Some("#000080"),
        "teal" => Some("#008080"),
        "olive" => Some("#808000"),
        "maroon" => Some("#800000"),
        "aqua" => Some("#00FFFF"),
        "silver" => Some("#C0C0C0"),
        "gold" => Some("#FFD700"),
        _ => None,
    }
}

fn transform_agent_for_opencode(content: &str) -> String {
    use std::borrow::Cow;

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return content.to_string();
    }

    let frontmatter = parts[1];
    let body = parts[2];

    let mut new_frontmatter = String::new();
    for line in frontmatter.lines() {
        if line.trim_start().starts_with("tools:") {
            let value = line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
            if !value.is_empty()
                && !value.starts_with('{')
                && !value.starts_with('\n')
                && value != "|"
            {
                new_frontmatter.push_str("tools:\n  \"*\": true\n");
                continue;
            }
        }

        if line.trim_start().starts_with("color:") {
            let value = line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
            let clean_value = value.trim_matches('"').trim_matches('\'');
            if !clean_value.is_empty()
                && !clean_value.starts_with('#')
                && let Some(hex) = color_name_to_hex(clean_value)
            {
                new_frontmatter.push_str(&format!("color: \"{}\"\n", hex));
                continue;
            }
        }

        new_frontmatter.push_str(line);
        new_frontmatter.push('\n');
    }

    format!("---\n{}\n---{}", new_frontmatter.trim_end(), body)
}

/// Canonical directory name for agents in profile storage.
const CANONICAL_AGENTS_DIR: &str = "agents";

/// Canonical directory name for commands in profile storage.
const CANONICAL_COMMANDS_DIR: &str = "commands";

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

    // For OpenCode, sanitize skill name and content before writing to profile
    // This ensures consistency between profile and harness (both use sanitized names)
    let kind = parse_harness_kind(&target.harness);
    let (skill_name, skill_content) = if matches!(kind, Some(HarnessKind::OpenCode)) {
        let sanitized = sanitize_name_for_opencode(&skill.name);
        let transformed = transform_skill_for_opencode(&skill.content, &sanitized);
        (sanitized, transformed)
    } else {
        (skill.name.clone(), skill.content.clone())
    };

    let skill_dir = profile_dir.join("skills").join(&skill_name);
    let skill_path = skill_dir.join("SKILL.md");

    if skill_path.exists() && !options.force {
        return Ok(InstallOutcome::Skipped(InstallSkip {
            skill: skill_name.clone(),
            target: target.clone(),
            reason: SkipReason::AlreadyExists,
        }));
    }

    fs::create_dir_all(&skill_dir).map_err(InstallError::CreateDir)?;
    fs::write(&skill_path, &skill_content).map_err(InstallError::WriteFile)?;

    if let Some(source_info) = source {
        update_manifest(&profile_dir, ComponentType::Skill, &skill_name, source_info);
    }

    let skill_for_harness = SkillInfo {
        name: skill_name.clone(),
        description: skill.description.clone(),
        path: skill.path.clone(),
        content: skill_content,
    };
    let harness_path = write_to_harness_if_active(target, &skill_for_harness)?;

    Ok(InstallOutcome::Installed(InstallSuccess {
        skill: skill_name,
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
    let (skill_dir_name, content) = if matches!(kind, HarnessKind::OpenCode) {
        let sanitized = sanitize_name_for_opencode(&skill.name);
        let transformed = transform_skill_for_opencode(&skill.content, &sanitized);
        (sanitized, transformed)
    } else {
        (skill.name.clone(), skill.content.clone())
    };
    let harness_skill_dir = skills_dir.join(&skill_dir_name);
    let harness_skill_path = harness_skill_dir.join("SKILL.md");

    fs::create_dir_all(&harness_skill_dir).map_err(InstallError::CreateDir)?;
    fs::write(&harness_skill_path, &content).map_err(InstallError::WriteFile)?;

    Ok(Some(harness_skill_path))
}

fn write_agent_to_harness_if_active(
    target: &InstallTarget,
    agent: &AgentInfo,
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

    // Check if harness supports agents - skip harness write if not
    let Some(agents_resource) = harness.agents(&Scope::Global).ok().flatten() else {
        return Ok(None);
    };
    let harness_agent_path = agents_resource.path.join(format!("{}.md", &agent.name));

    if let Some(parent) = harness_agent_path.parent() {
        fs::create_dir_all(parent).map_err(InstallError::CreateDir)?;
    }

    let content = if matches!(kind, HarnessKind::OpenCode) {
        transform_agent_for_opencode(&agent.content)
    } else {
        agent.content.clone()
    };
    fs::write(&harness_agent_path, &content).map_err(InstallError::WriteFile)?;

    Ok(Some(harness_agent_path))
}

fn write_command_to_harness_if_active(
    target: &InstallTarget,
    command: &CommandInfo,
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

    let Some(commands_resource) = harness.commands(&Scope::Global).ok().flatten() else {
        return Ok(None);
    };
    let harness_command_path = commands_resource.path.join(format!("{}.md", &command.name));

    if let Some(parent) = harness_command_path.parent() {
        fs::create_dir_all(parent).map_err(InstallError::CreateDir)?;
    }
    fs::write(&harness_command_path, &command.content).map_err(InstallError::WriteFile)?;

    Ok(Some(harness_command_path))
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
    let profiles_dir = BridleConfig::profiles_dir().map_err(|_| InstallError::ProfileNotFound {
        harness: target.harness.clone(),
        profile: target.profile.as_str().to_string(),
    })?;
    install_agent_to_dir(agent, target, options, &profiles_dir)
}

pub fn install_agent_to_dir(
    agent: &AgentInfo,
    target: &InstallTarget,
    options: &InstallOptions,
    profiles_dir: &Path,
) -> InstallResult {
    install_agent_to_dir_with_source(agent, target, options, profiles_dir, None)
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
    install_agent_to_dir_with_source(agent, target, options, &profiles_dir, source)
}

fn install_agent_to_dir_with_source(
    agent: &AgentInfo,
    target: &InstallTarget,
    options: &InstallOptions,
    profiles_dir: &Path,
    source: Option<&SourceInfo>,
) -> InstallResult {
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

    let agents_dir = profile_dir.join(CANONICAL_AGENTS_DIR);
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

    let harness_path = write_agent_to_harness_if_active(target, agent)?;

    Ok(InstallOutcome::Installed(InstallSuccess {
        skill: agent.name.clone(),
        target: target.clone(),
        profile_path: agent_path,
        harness_path,
    }))
}

pub fn install_command(
    command: &CommandInfo,
    target: &InstallTarget,
    options: &InstallOptions,
) -> InstallResult {
    let profiles_dir = BridleConfig::profiles_dir().map_err(|_| InstallError::ProfileNotFound {
        harness: target.harness.clone(),
        profile: target.profile.as_str().to_string(),
    })?;
    install_command_to_dir(command, target, options, &profiles_dir)
}

pub fn install_command_to_dir(
    command: &CommandInfo,
    target: &InstallTarget,
    options: &InstallOptions,
    profiles_dir: &Path,
) -> InstallResult {
    install_command_to_dir_with_source(command, target, options, profiles_dir, None)
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
    install_command_to_dir_with_source(command, target, options, &profiles_dir, source)
}

fn install_command_to_dir_with_source(
    command: &CommandInfo,
    target: &InstallTarget,
    options: &InstallOptions,
    profiles_dir: &Path,
    source: Option<&SourceInfo>,
) -> InstallResult {
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

    let commands_dir = profile_dir.join(CANONICAL_COMMANDS_DIR);
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

    let harness_path = write_command_to_harness_if_active(target, command)?;

    Ok(InstallOutcome::Installed(InstallSuccess {
        skill: command.name.clone(),
        target: target.clone(),
        profile_path: command_path,
        harness_path,
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
            assert!(
                content.contains("name: my-skill"),
                "OpenCode profile should have sanitized frontmatter"
            );
            assert!(
                content.contains("# My Skill"),
                "Content body should be preserved"
            );
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
        assert!(
            content.contains("new content"),
            "New content should be present"
        );
        assert!(
            content.contains("name: existing"),
            "OpenCode profile should have frontmatter"
        );
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

    #[test]
    fn install_agent_uses_canonical_agents_dir() {
        let (_temp, target, profiles_dir) = setup_test_env();

        let agent = AgentInfo {
            name: "test-agent".to_string(),
            description: None,
            path: "agents/test-agent.md".to_string(),
            content: "# Test Agent".to_string(),
        };

        let result =
            install_agent_to_dir(&agent, &target, &InstallOptions::default(), &profiles_dir);
        assert!(result.is_ok());

        if let Ok(InstallOutcome::Installed(success)) = result {
            assert!(
                success.profile_path.to_string_lossy().contains("/agents/"),
                "Expected path to contain '/agents/', got: {:?}",
                success.profile_path
            );
        }
    }

    #[test]
    fn install_command_uses_canonical_commands_dir() {
        let (_temp, target, profiles_dir) = setup_test_env();

        let command = CommandInfo {
            name: "test-command".to_string(),
            description: None,
            path: "commands/test-command.md".to_string(),
            content: "# Test Command".to_string(),
        };

        let result =
            install_command_to_dir(&command, &target, &InstallOptions::default(), &profiles_dir);
        assert!(result.is_ok());

        if let Ok(InstallOutcome::Installed(success)) = result {
            assert!(
                success
                    .profile_path
                    .to_string_lossy()
                    .contains("/commands/"),
                "Expected path to contain '/commands/', got: {:?}",
                success.profile_path
            );
        }
    }

    #[test]
    fn install_sanitizes_skill_name_for_opencode() {
        let (_temp, target, profiles_dir) = setup_test_env();

        let skill = SkillInfo {
            name: "Hook Development".to_string(),
            description: Some("A skill with spaces".to_string()),
            path: "skills/Hook Development/SKILL.md".to_string(),
            content: "---\nname: Hook Development\ndescription: Test\n---\n# Content".to_string(),
        };

        let result =
            install_skill_to_dir(&skill, &target, &InstallOptions::default(), &profiles_dir);
        assert!(result.is_ok());

        if let Ok(InstallOutcome::Installed(success)) = result {
            assert!(
                success
                    .profile_path
                    .to_string_lossy()
                    .contains("/hook-development/"),
                "Expected sanitized path with 'hook-development', got: {:?}",
                success.profile_path
            );

            let content = fs::read_to_string(&success.profile_path).unwrap();
            assert!(
                content.contains("name: hook-development"),
                "Expected frontmatter name to be sanitized, got: {}",
                content
            );
        }
    }
}
