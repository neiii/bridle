//! Types for installation operations.

use std::collections::HashMap;
use std::path::PathBuf;

use harness_locate::McpServer;
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

/// Information about a discovered agent
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub description: Option<String>,
    pub path: String,
    pub content: String,
}

/// Information about a discovered command
#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub name: String,
    pub description: Option<String>,
    pub path: String,
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
    /// Discovered MCP servers (name -> server config)
    pub mcp_servers: HashMap<String, McpServer>,
    /// Discovered agents
    pub agents: Vec<AgentInfo>,
    /// Discovered commands
    pub commands: Vec<CommandInfo>,
    /// Source repository metadata
    pub source: SourceInfo,
}

/// Metadata about the source repository
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Serialize)]
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

/// Component type for uninstall operations
#[derive(Debug, Clone, Copy, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentType {
    Skill,
    Agent,
    Command,
}

impl ComponentType {
    pub fn dir_name(&self) -> &'static str {
        match self {
            ComponentType::Skill => "skills",
            ComponentType::Agent => "agents",
            ComponentType::Command => "commands",
        }
    }
}

/// Result of uninstallation operation
#[derive(Debug, Default, Serialize)]
pub struct UninstallReport {
    pub removed: Vec<UninstallSuccess>,
    pub errors: Vec<UninstallFailure>,
}

#[derive(Debug, Serialize)]
pub struct UninstallSuccess {
    /// Component name
    pub component: String,
    /// Component type
    pub component_type: String,
    /// Where it was removed from
    pub target: InstallTarget,
    /// Profile path that was removed
    pub profile_path: PathBuf,
    /// Harness path that was removed (if active profile)
    pub harness_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct UninstallFailure {
    pub component: String,
    pub component_type: String,
    pub target: InstallTarget,
    pub error: String,
}
