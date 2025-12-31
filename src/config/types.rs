//! Shared types for profile management.

use std::path::PathBuf;

use serde::Serialize;

/// MCP server info with enabled status and connection details.
#[derive(Debug, Clone, Default, Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub enabled: bool,
    pub server_type: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
}

/// Summary of directory-based resources (skills, commands, etc.).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ResourceSummary {
    /// List of resource names/items.
    pub items: Vec<String>,
    /// Whether the resource directory exists.
    pub directory_exists: bool,
}

/// Information about a profile for display purposes.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProfileInfo {
    /// Profile name.
    pub name: String,
    /// Harness identifier.
    pub harness_id: String,
    /// Whether this is the currently active profile.
    pub is_active: bool,
    /// Path to the profile directory.
    pub path: PathBuf,

    /// MCP servers with enabled status.
    pub mcp_servers: Vec<McpServerInfo>,

    /// Skills directory summary.
    pub skills: ResourceSummary,
    /// Commands directory summary.
    pub commands: ResourceSummary,
    /// Plugins directory summary (OpenCode only).
    pub plugins: Option<ResourceSummary>,
    /// Agents directory summary (OpenCode only).
    pub agents: Option<ResourceSummary>,
    /// Path to rules file if it exists.
    pub rules_file: Option<PathBuf>,
    /// Theme setting (OpenCode only).
    pub theme: Option<String>,
    /// Model setting.
    pub model: Option<String>,
    /// Errors encountered during extraction.
    pub extraction_errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_server_info_default() {
        let info = McpServerInfo::default();
        assert!(info.name.is_empty());
        assert!(!info.enabled);
        assert!(info.server_type.is_none());
    }

    #[test]
    fn resource_summary_default() {
        let summary = ResourceSummary::default();
        assert!(summary.items.is_empty());
        assert!(!summary.directory_exists);
    }

    #[test]
    fn profile_info_default() {
        let info = ProfileInfo::default();
        assert!(info.name.is_empty());
        assert!(!info.is_active);
        assert!(info.mcp_servers.is_empty());
    }

    #[test]
    fn types_serialize_to_json() {
        let info = ProfileInfo {
            name: "test".to_string(),
            harness_id: "opencode".to_string(),
            is_active: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&info).expect("should serialize");
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"is_active\":true"));
    }
}
