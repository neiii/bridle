//! Grok Build harness implementation.
//!
//! Grok Build stores its configuration in `.grok/` directories.

use std::path::PathBuf;

use crate::error::Result;
use crate::mcp::McpServer;
use crate::types::Scope;

use super::dot_dir::DotDirHarness;
use super::mcp_parse::ParseConfig;

const HARNESS: DotDirHarness = DotDirHarness {
    dot_dir: ".grok",
    agents_dir: "agents",
    mcp_key: "mcp",
    mcp_config: &ParseConfig::GROK_BUILD,
};

/// Returns the global Grok Build configuration directory.
pub fn global_config_dir() -> Result<PathBuf> {
    HARNESS.global_config_dir()
}

/// Returns the project-local Grok Build configuration directory.
#[must_use]
pub fn project_config_dir(project_root: &std::path::Path) -> PathBuf {
    HARNESS.project_config_dir(project_root)
}

/// Returns the config directory for the given scope.
pub fn config_dir(scope: &Scope) -> Result<PathBuf> {
    HARNESS.config_dir(scope)
}

/// Returns the commands directory for the given scope.
pub fn commands_dir(scope: &Scope) -> Result<PathBuf> {
    HARNESS.child_dir(scope, "commands")
}

/// Returns the MCP configuration directory for the given scope.
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
}

/// Returns the skills directory for the given scope.
#[must_use]
pub fn skills_dir(scope: &Scope) -> Option<PathBuf> {
    HARNESS.optional_child_dir(scope, "skills")
}

/// Returns the rules directory for the given scope.
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    HARNESS.rules_dir(scope)
}

/// Returns the agents directory for the given scope.
#[must_use]
pub fn agents_dir(scope: &Scope) -> Option<PathBuf> {
    HARNESS.agents_dir(scope)
}

/// Checks if Grok Build is installed on this system.
pub fn is_installed() -> bool {
    HARNESS.is_installed()
}

/// Parses a single MCP server from Grok Build's native format.
pub(crate) fn parse_mcp_server(value: &serde_json::Value) -> Result<McpServer> {
    HARNESS.parse_mcp_server(value)
}

/// Parses all MCP servers from a Grok Build config JSON.
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    HARNESS.parse_mcp_servers(config, parse_mcp_server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform;
    use crate::types::EnvValue;
    use serde_json::json;

    #[test]
    fn directories_use_expected_names() {
        let root = PathBuf::from("/some/project");
        assert_eq!(
            project_config_dir(&root),
            PathBuf::from("/some/project/.grok")
        );
        assert_eq!(
            commands_dir(&Scope::Project(root.clone())).unwrap(),
            PathBuf::from("/some/project/.grok/commands")
        );
        assert_eq!(
            skills_dir(&Scope::Project(root.clone())).unwrap(),
            PathBuf::from("/some/project/.grok/skills")
        );
        assert_eq!(
            agents_dir(&Scope::Project(root.clone())).unwrap(),
            PathBuf::from("/some/project/.grok/agents")
        );
        assert_eq!(rules_dir(&Scope::Project(root.clone())).unwrap(), root);
    }

    #[test]
    fn global_config_dir_is_absolute() {
        if platform::home_dir().is_err() {
            return;
        }
        let path = global_config_dir().unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with(".grok"));
    }

    #[test]
    fn parses_stdio_http_and_sse_servers() {
        assert!(matches!(
            parse_mcp_server(&json!({"command":"npx","args":["-y","pkg"]})).unwrap(),
            McpServer::Stdio(_)
        ));
        assert!(matches!(
            parse_mcp_server(&json!({"type":"http","url":"https://api.example.com/mcp"})).unwrap(),
            McpServer::Http(_)
        ));
        assert!(matches!(
            parse_mcp_server(&json!({"url":"https://example.com/sse"})).unwrap(),
            McpServer::Sse(_)
        ));
    }

    #[test]
    fn parses_stdio_options() {
        let server = parse_mcp_server(&json!({
            "command": "node",
            "args": ["server.js"],
            "env": { "API_KEY": "${MY_API_KEY}" },
            "timeout": 30000,
            "disabled": true
        }))
        .unwrap();
        let McpServer::Stdio(server) = server else {
            panic!("Expected Stdio variant");
        };
        assert_eq!(
            server.env.get("API_KEY"),
            Some(&EnvValue::env("MY_API_KEY"))
        );
        assert_eq!(server.timeout_ms, Some(30000));
        assert!(!server.enabled);
    }

    #[test]
    fn parses_server_map() {
        let config = json!({ "mcp": {
            "filesystem": { "command": "npx" },
            "remote-server": { "url": "https://example.com/sse" }
        } });
        let servers = parse_mcp_servers(&config).unwrap();
        assert_eq!(servers.len(), 2);
    }

    #[test]
    fn invalid_mcp_config_fails() {
        assert!(parse_mcp_server(&json!({"args":["server.js"]})).is_err());
        assert!(parse_mcp_server(&json!({"type":"unknown","url":"https://example.com"})).is_err());
        assert!(parse_mcp_servers(&json!({"other":"data"})).is_err());
    }
}
