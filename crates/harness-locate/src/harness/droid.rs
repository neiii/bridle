//! Factory Droid harness implementation.
//!
//! Factory Droid stores its configuration in:
//! - **Global**: `~/.factory/`
//! - **Project**: `.factory/` in project root

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::mcp::McpServer;
use crate::platform;
use crate::types::Scope;

use super::mcp_parse::{self, ParseConfig};

/// Returns the global Droid configuration directory.
///
/// Returns `~/.factory/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn global_config_dir() -> Result<PathBuf> {
    Ok(platform::home_dir()?.join(".factory"))
}

/// Returns the project-local Droid configuration directory.
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
#[must_use]
pub fn project_config_dir(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".factory")
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

/// Returns the commands directory for the given scope.
///
/// - **Global**: `~/.factory/commands/`
/// - **Project**: `.factory/commands/`
pub fn commands_dir(scope: &Scope) -> Result<PathBuf> {
    match scope {
        Scope::Global => Ok(global_config_dir()?.join("commands")),
        Scope::Project(root) => Ok(project_config_dir(root).join("commands")),
        Scope::Custom(path) => Ok(path.join("commands")),
    }
}

/// Returns the MCP configuration directory for the given scope.
///
/// Droid stores MCP configuration in `mcp.json` at the base config directory.
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
}

/// Returns the skills directory for the given scope.
///
/// Droid stores skills in nested directories with `SKILL.md` files:
/// - **Global**: `~/.factory/skills/`
/// - **Project**: `.factory/skills/`
#[must_use]
pub fn skills_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok().map(|p| p.join("skills")),
        Scope::Project(root) => Some(project_config_dir(root).join("skills")),
        Scope::Custom(path) => Some(path.join("skills")),
    }
}

/// Returns the rules directory for the given scope.
///
/// Droid stores rules files at:
/// - **Global**: `~/.factory/`
/// - **Project**: Project root directory (not `.factory/`)
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok(),
        Scope::Project(root) => Some(root.clone()),
        Scope::Custom(path) => Some(path.clone()),
    }
}

/// Returns the agents (droids) directory for the given scope.
///
/// Droid stores agents as markdown files with YAML frontmatter:
/// - **Global**: `~/.factory/droids/`
/// - **Project**: `.factory/droids/`
#[must_use]
pub fn agents_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok().map(|p| p.join("droids")),
        Scope::Project(root) => Some(project_config_dir(root).join("droids")),
        Scope::Custom(path) => Some(path.join("droids")),
    }
}

/// Checks if Droid is installed on this system.
///
/// Currently checks if the global config directory exists.
pub fn is_installed() -> bool {
    global_config_dir().map(|p| p.exists()).unwrap_or(false)
}

/// Parses a single MCP server from Droid's native JSON format.
///
/// # Arguments
/// * `value` - The JSON value representing the server config
///
/// # Errors
/// Returns an error if the JSON is malformed or missing required fields.
pub(crate) fn parse_mcp_server(value: &serde_json::Value) -> Result<McpServer> {
    let config = ParseConfig::DROID;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: "Server configuration must be an object".to_string(),
        })?;

    // Check if this is an SSE or HTTP server (has "type" field)
    if let Some(server_type) = obj.get("type").and_then(|v| v.as_str()) {
        match server_type {
            "http" => mcp_parse::parse_http_server(obj, &config),
            "stdio" => mcp_parse::parse_stdio_server(obj, &config),
            _ => Err(Error::UnsupportedMcpConfig {
                harness: config.harness_name.to_string(),
                reason: format!("Unknown server type: {}", server_type),
            }),
        }
    } else if obj.contains_key("url") {
        // SSE server (remote without explicit type, or with url field)
        mcp_parse::parse_sse_server(obj, &config)
    } else {
        mcp_parse::parse_stdio_server(obj, &config)
    }
}

/// Parses all MCP servers from a Droid config JSON.
///
/// # Arguments
/// * `config` - The full config JSON (expects mcpServers key)
///
/// # Errors
/// Returns an error if the JSON is malformed.
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    mcp_parse::parse_servers_from_key(config, "mcpServers", &ParseConfig::DROID, parse_mcp_server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EnvValue;
    use serde_json::json;

    #[test]
    fn global_config_dir_is_absolute() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = global_config_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with(".factory"));
    }

    #[test]
    fn project_config_dir_is_relative_to_root() {
        let root = PathBuf::from("/some/project");
        let config = project_config_dir(&root);
        assert_eq!(config, PathBuf::from("/some/project/.factory"));
    }

    #[test]
    fn commands_dir_global() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = commands_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("commands"));
    }

    #[test]
    fn commands_dir_project() {
        let root = PathBuf::from("/some/project");
        let result = commands_dir(&Scope::Project(root));
        assert!(result.is_ok());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.factory/commands"));
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
    fn skills_dir_project() {
        let root = PathBuf::from("/some/project");
        let result = skills_dir(&Scope::Project(root));
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.factory/skills"));
    }

    #[test]
    fn rules_dir_global_returns_config_dir() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = rules_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with(".factory"));
    }

    #[test]
    fn rules_dir_project_returns_root() {
        let root = PathBuf::from("/some/project");
        let result = rules_dir(&Scope::Project(root.clone()));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), root);
    }

    #[test]
    fn agents_dir_returns_droids_path() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = agents_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with("droids"));
    }

    #[test]
    fn agents_dir_project() {
        let root = PathBuf::from("/some/project");
        let result = agents_dir(&Scope::Project(root));
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.factory/droids"));
    }

    #[test]
    fn parse_stdio_server_basic() {
        let json = json!({
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"]
        });

        let result = parse_mcp_server(&json);
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
        let json = json!({
            "command": "node",
            "args": ["server.js"],
            "env": {
                "API_KEY": "${MY_API_KEY}",
                "DEBUG": "true"
            }
        });

        let result = parse_mcp_server(&json);
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
    fn parse_stdio_server_with_disabled() {
        let json = json!({
            "command": "node",
            "args": ["server.js"],
            "disabled": true
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert!(!server.enabled);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_with_timeout() {
        let json = json!({
            "command": "node",
            "args": ["server.js"],
            "timeout": 30000
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.timeout_ms, Some(30000));
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_http_server_basic() {
        let json = json!({
            "type": "http",
            "url": "https://api.example.com/mcp"
        });

        let result = parse_mcp_server(&json);
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
    fn parse_http_server_with_headers() {
        let json = json!({
            "type": "http",
            "url": "https://api.example.com/mcp",
            "headers": {
                "X-API-Key": "${API_KEY}"
            }
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_ok());

        if let McpServer::Http(server) = result.unwrap() {
            assert_eq!(server.url, "https://api.example.com/mcp");
            assert_eq!(server.headers.len(), 1);
            assert_eq!(
                server.headers.get("X-API-Key"),
                Some(&EnvValue::env("API_KEY"))
            );
        } else {
            panic!("Expected Http variant");
        }
    }

    #[test]
    fn parse_sse_server_with_url_only() {
        let json = json!({
            "url": "https://example.com/sse"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_ok());

        if let McpServer::Sse(server) = result.unwrap() {
            assert_eq!(server.url, "https://example.com/sse");
            assert!(server.headers.is_empty());
            assert!(server.enabled);
        } else {
            panic!("Expected Sse variant");
        }
    }

    #[test]
    fn parse_sse_server_with_headers() {
        let json = json!({
            "url": "https://example.com/sse",
            "headers": {
                "Authorization": "${TOKEN}"
            }
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_ok());

        if let McpServer::Sse(server) = result.unwrap() {
            assert_eq!(server.url, "https://example.com/sse");
            assert_eq!(server.headers.len(), 1);
            assert_eq!(
                server.headers.get("Authorization"),
                Some(&EnvValue::env("TOKEN"))
            );
        } else {
            panic!("Expected Sse variant");
        }
    }

    #[test]
    fn parse_mcp_server_missing_command_fails() {
        let json = json!({
            "args": ["server.js"]
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_missing_url_for_http_fails() {
        let json = json!({
            "type": "http"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_unknown_type_fails() {
        let json = json!({
            "type": "unknown",
            "url": "https://example.com"
        });

        let result = parse_mcp_server(&json);
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
                "remote-server": {
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

        let remote_server = servers
            .iter()
            .find(|(name, _)| name == "remote-server")
            .unwrap();
        assert!(matches!(remote_server.1, McpServer::Sse(_)));

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
