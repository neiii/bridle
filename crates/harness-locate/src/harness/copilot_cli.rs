//! GitHub Copilot CLI harness implementation.
//!
//! GitHub Copilot CLI (`@github/copilot` npm package) stores its configuration in:
//! - **Global**: `$XDG_CONFIG_HOME/copilot` or `~/.copilot/`
//! - **Project**: `.github/` in project root

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::mcp::McpServer;
use crate::platform;
use crate::types::Scope;

use super::mcp_parse::{self, ParseConfig};

/// Environment variable for XDG config directory override.
const XDG_CONFIG_HOME_ENV: &str = "XDG_CONFIG_HOME";

/// Returns the global Copilot CLI configuration directory.
///
/// Checks `XDG_CONFIG_HOME` environment variable first (returns `$XDG_CONFIG_HOME/copilot`),
/// then falls back to `~/.copilot/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined and
/// no environment variable is set.
pub fn global_config_dir() -> Result<PathBuf> {
    // Check XDG_CONFIG_HOME first
    if let Ok(xdg_config) = std::env::var(XDG_CONFIG_HOME_ENV) {
        let path = PathBuf::from(xdg_config);
        if path.is_absolute() {
            return Ok(path.join("copilot"));
        }
    }

    // Fall back to ~/.copilot/
    Ok(platform::home_dir()?.join(".copilot"))
}

/// Returns the project-local Copilot CLI configuration directory.
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
#[must_use]
pub fn project_config_dir(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".github")
}

/// Returns the config directory for the given scope.
///
/// This is the base configuration directory.
pub fn config_dir(scope: &Scope) -> Result<PathBuf> {
    match scope {
        Scope::Global => global_config_dir(),
        Scope::Project(root) => Ok(project_config_dir(root)),
        Scope::Custom(path) => Ok(path.clone()),
    }
}

/// Returns the MCP configuration directory for the given scope.
///
/// Copilot CLI stores MCP configuration in `mcp-config.json`.
/// Note that project-local MCP configuration is not natively supported.
/// - **Global**: `~/.copilot/mcp-config.json`
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    match scope {
        Scope::Global => global_config_dir(),
        Scope::Project(_) => Err(Error::UnsupportedScope {
            harness: "Copilot CLI".to_string(),
            scope: "project".to_string(),
        }),
        Scope::Custom(path) => Ok(path.clone()),
    }
}

/// Returns the skills directory for the given scope.
///
/// Copilot CLI stores skills following the agentskills.io spec.
/// Note that project-local skills are not yet natively supported in the CLI.
/// - **Global**: `~/.copilot/skills/`
#[must_use]
pub fn skills_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok().map(|p| p.join("skills")),
        Scope::Project(_) => None,
        Scope::Custom(path) => Some(path.join("skills")),
    }
}

/// Returns the agents directory for the given scope.
///
/// Copilot CLI stores agents as Markdown files with YAML frontmatter:
/// - **Global**: `~/.copilot/agents/`
/// - **Project**: `.github/agents/`
#[must_use]
pub fn agents_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok().map(|p| p.join("agents")),
        Scope::Project(root) => Some(project_config_dir(root).join("agents")),
        Scope::Custom(path) => Some(path.join("agents")),
    }
}

/// Returns the rules directory for the given scope.
///
/// Copilot CLI stores rules files (`copilot-instructions.md`) at:
/// - **Global**: `~/.copilot/`
/// - **Project**: `.github/`
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok(),
        Scope::Project(root) => Some(project_config_dir(root)),
        Scope::Custom(path) => Some(path.clone()),
    }
}

/// Checks if Copilot CLI is installed on this system.
///
/// Checks for the `copilot` binary or the existence of `~/.copilot/`.
pub fn is_installed() -> bool {
    // Check for copilot binary
    let copilot_exists = std::process::Command::new("copilot")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if copilot_exists {
        return true;
    }

    // Fallback to checking for ~/.copilot directory
    global_config_dir().map(|p| p.exists()).unwrap_or(false)
}

/// Parses a single MCP server from Copilot CLI's native JSON format.
///
/// Copilot CLI uses the same format as Claude Code (mcpServers key, ${VAR} syntax).
///
/// # Arguments
/// * `value` - The JSON value representing the server config
///
/// # Errors
/// Returns an error if the JSON is malformed or missing required fields.
pub(crate) fn parse_mcp_server(value: &serde_json::Value) -> Result<McpServer> {
    let config = ParseConfig::COPILOT_CLI;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: "Server configuration must be an object".to_string(),
        })?;

    // Check if this is an SSE or HTTP server (has "type" field)
    if let Some(server_type) = obj.get("type").and_then(|v| v.as_str()) {
        match server_type {
            "sse" => mcp_parse::parse_sse_server(obj, &config),
            "http" => mcp_parse::parse_http_server(obj, &config),
            "stdio" | "local" => mcp_parse::parse_stdio_server(obj, &config),
            _ => Err(Error::UnsupportedMcpConfig {
                harness: config.harness_name.to_string(),
                reason: format!("Unknown server type: {}", server_type),
            }),
        }
    } else {
        mcp_parse::parse_stdio_server(obj, &config)
    }
}

/// Parses all MCP servers from a Copilot CLI config JSON.
///
/// # Arguments
/// * `config` - The full config JSON (expects mcpServers key)
///
/// # Errors
/// Returns an error if the JSON is malformed.
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    mcp_parse::parse_servers_from_key(
        config,
        "mcpServers",
        &ParseConfig::COPILOT_CLI,
        parse_mcp_server,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EnvValue;
    use serde_json::json;

    #[test]
    fn global_config_dir_is_absolute() {
        // Skip if home dir cannot be determined (CI environments)
        if platform::home_dir().is_err() {
            return;
        }

        let result = global_config_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.is_absolute());
        // When XDG_CONFIG_HOME is set, ends with "copilot"; otherwise ".copilot"
        assert!(
            path.ends_with(std::path::Path::new("copilot"))
                || path.ends_with(std::path::Path::new(".copilot"))
        );
    }

    #[test]
    fn project_config_dir_is_relative_to_root() {
        let root = PathBuf::from("/some/project");
        let config = project_config_dir(&root);
        assert_eq!(config, PathBuf::from("/some/project/.github"));
    }

    #[test]
    fn skills_dir_global() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = skills_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with("skills"));
    }

    #[test]
    fn skills_dir_project_unsupported() {
        let root = PathBuf::from("/some/project");
        let result = skills_dir(&Scope::Project(root));
        assert!(result.is_none());
    }

    #[test]
    fn agents_dir_global() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = agents_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with("agents"));
    }

    #[test]
    fn agents_dir_project_uses_github_dir() {
        let root = PathBuf::from("/some/project");
        let result = agents_dir(&Scope::Project(root));
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.github/agents"));
    }

    #[test]
    fn rules_dir_global_returns_config_dir() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = rules_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        // When XDG_CONFIG_HOME is set, ends with "copilot"; otherwise ".copilot"
        assert!(
            path.ends_with(std::path::Path::new("copilot"))
                || path.ends_with(std::path::Path::new(".copilot"))
        );
    }

    #[test]
    fn rules_dir_project_uses_github_dir() {
        let root = PathBuf::from("/some/project");
        let result = rules_dir(&Scope::Project(root));
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.github"));
    }

    #[test]
    fn parse_stdio_server_basic() {
        let json_val = json!({
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"]
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.command, "npx");
            assert_eq!(server.args.len(), 2);
            assert_eq!(server.args[0], "-y");
            assert_eq!(server.args[1], "@modelcontextprotocol/server-filesystem");
            assert!(server.env.is_empty());
            assert!(server.enabled);
            assert_eq!(server.timeout_ms, None);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_with_env() {
        let json_val = json!({
            "command": "node",
            "args": ["server.js"],
            "env": {
                "API_KEY": "${MY_API_KEY}",
                "DEBUG": "true"
            }
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.command, "node");
            assert_eq!(server.env.len(), 2);
            assert_eq!(
                server.env.get("API_KEY"),
                Some(&EnvValue::env("MY_API_KEY"))
            );
            assert_eq!(server.env.get("DEBUG"), Some(&EnvValue::plain("true")));
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_with_timeout() {
        let json_val = json!({
            "command": "node",
            "args": ["server.js"],
            "timeout": 30000
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.timeout_ms, Some(30000));
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_local_server_basic() {
        // Test that "local" type is recognized as stdio server
        let json_val = json!({
            "type": "local",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"]
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.command, "npx");
            assert_eq!(server.args.len(), 2);
            assert_eq!(server.args[0], "-y");
            assert_eq!(server.args[1], "@modelcontextprotocol/server-filesystem");
            assert!(server.env.is_empty());
            assert!(server.enabled);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_local_server_with_env() {
        let json_val = json!({
            "type": "local",
            "command": "node",
            "args": ["server.js"],
            "env": {
                "API_KEY": "${MY_API_KEY}",
                "DEBUG": "true"
            }
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.command, "node");
            assert_eq!(server.env.len(), 2);
            assert_eq!(
                server.env.get("API_KEY"),
                Some(&EnvValue::env("MY_API_KEY"))
            );
            assert_eq!(server.env.get("DEBUG"), Some(&EnvValue::plain("true")));
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_sse_server_basic() {
        let json_val = json!({
            "type": "sse",
            "url": "https://example.com/sse"
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_ok());

        if let McpServer::Sse(server) = result.unwrap() {
            assert_eq!(server.url, "https://example.com/sse");
            assert!(server.headers.is_empty());
            assert!(server.enabled);
            assert_eq!(server.timeout_ms, None);
        } else {
            panic!("Expected Sse variant");
        }
    }

    #[test]
    fn parse_sse_server_with_headers() {
        let json_val = json!({
            "type": "sse",
            "url": "https://example.com/sse",
            "headers": {
                "Authorization": "${TOKEN}",
                "X-Custom-Header": "value"
            }
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_ok());

        if let McpServer::Sse(server) = result.unwrap() {
            assert_eq!(server.url, "https://example.com/sse");
            assert_eq!(server.headers.len(), 2);
            assert_eq!(
                server.headers.get("Authorization"),
                Some(&EnvValue::env("TOKEN"))
            );
            assert_eq!(
                server.headers.get("X-Custom-Header"),
                Some(&EnvValue::plain("value"))
            );
        } else {
            panic!("Expected Sse variant");
        }
    }

    #[test]
    fn parse_http_server_basic() {
        let json_val = json!({
            "type": "http",
            "url": "https://api.example.com/mcp"
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_ok());

        if let McpServer::Http(server) = result.unwrap() {
            assert_eq!(server.url, "https://api.example.com/mcp");
            assert!(server.headers.is_empty());
            assert!(server.oauth.is_none());
            assert!(server.enabled);
            assert_eq!(server.timeout_ms, None);
        } else {
            panic!("Expected Http variant");
        }
    }

    #[test]
    fn parse_mcp_server_missing_command_fails() {
        let json_val = json!({
            "args": ["server.js"]
        });

        let result = parse_mcp_server(&json_val);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_full_config() {
        let config = json!({
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem"],
                    "env": {
                        "ROOT_DIR": "${HOME}"
                    }
                },
                "sse-server": {
                    "type": "sse",
                    "url": "https://example.com/sse",
                    "headers": {
                        "Authorization": "${TOKEN}"
                    }
                },
                "http-server": {
                    "type": "http",
                    "url": "https://api.example.com/mcp"
                }
            }
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_ok());

        let servers = result.unwrap();
        assert_eq!(servers.len(), 3);

        let filesystem = servers
            .iter()
            .find(|(name, _)| name == "filesystem")
            .unwrap();
        assert!(matches!(filesystem.1, McpServer::Stdio(_)));

        let sse_server = servers
            .iter()
            .find(|(name, _)| name == "sse-server")
            .unwrap();
        assert!(matches!(sse_server.1, McpServer::Sse(_)));

        let http_server = servers
            .iter()
            .find(|(name, _)| name == "http-server")
            .unwrap();
        assert!(matches!(http_server.1, McpServer::Http(_)));
    }

    #[test]
    fn parse_mcp_servers_empty_config() {
        let config = json!({
            "mcpServers": {}
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_mcp_servers_missing_mcp_servers_key_fails() {
        let config = json!({
            "other": "data"
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_err());
    }
}
