//! Types for installation operations.

use std::path::PathBuf;

use serde::Serialize;

use crate::config::ProfileName;

/// Information about a discovered skill
#[derive(Debug, Clone)]
pub struct SkillInfo {
    /// Skill name (from SKILL.md frontmatter)
    pub name: String,
    /// Skill description (from SKILL.md frontmatter)
    pub description: Option<String>,
    /// Path within source archive (e.g., "skills/memory-safety/SKILL.md")
    pub path: String,
    /// Actual SKILL.md file content
    pub content: String,
}

/// Target harness + profile for installation
#[derive(Debug, Clone, Serialize)]
pub struct InstallTarget {
    /// Harness identifier (e.g., "opencode", "claude-code")
    pub harness: String,
    pub profile: ProfileName,
}

/// Options controlling installation behavior
#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    /// Overwrite existing files
    pub force: bool,
}

/// Result of discovery operation
#[derive(Debug)]
pub struct DiscoveryResult {
    /// Discovered skills
    pub skills: Vec<SkillInfo>,
    /// Source repository metadata
    pub source: SourceInfo,
}

/// Metadata about the source repository
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub owner: String,
    pub repo: String,
    pub git_ref: Option<String>,
}

/// Result of installation operation
#[derive(Debug, Default, Serialize)]
pub struct InstallReport {
    pub installed: Vec<InstallSuccess>,
    pub skipped: Vec<InstallSkip>,
    pub errors: Vec<InstallFailure>,
}

#[derive(Debug, Serialize)]
pub struct InstallSuccess {
    /// Component name
    pub skill: String,
    /// Where it was installed
    pub target: InstallTarget,
    /// Path in profile storage
    pub profile_path: PathBuf,
    /// Path in harness config (None if profile not active)
    pub harness_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct InstallSkip {
    pub skill: String,
    pub target: InstallTarget,
    pub reason: SkipReason,
}

#[derive(Debug, Serialize)]
pub enum SkipReason {
    /// File already exists and --force not specified
    AlreadyExists,
}

#[derive(Debug, Serialize)]
pub struct InstallFailure {
    pub skill: String,
    pub target: InstallTarget,
    pub error: String,
}
