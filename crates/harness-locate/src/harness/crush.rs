//! Crush harness implementation.
//!
//! Crush (Charmbracelet's AI coding assistant) stores its configuration in:
//! - **Global**: `~/.config/crush/`
//! - **Project**: `.crush/` in project root (if exists)

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::mcp::McpServer;
use crate::platform;
use crate::types::Scope;

use super::mcp_parse::{self, ParseConfig};

/// Returns the global Crush configuration directory.
///
/// Returns `~/.config/crush/` on all platforms.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn global_config_dir() -> Result<PathBuf> {
    Ok(platform::config_dir()?.join("crush"))
}

/// Returns the project-local Crush configuration directory.
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
#[must_use]
pub fn project_config_dir(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".crush")
}

/// Returns the commands directory for the given scope.
///
/// Crush does not have a dedicated commands directory, so this
/// returns the config directory itself.
pub fn commands_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
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
/// Crush stores MCP configuration in the base config directory.
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
}

/// Returns the skills directory path for Crush.
///
/// Crush stores skills in:
/// - Global: `~/.config/crush/skills/`
/// - Project: `.crush/skills/`
///
/// Skills use plain Markdown format (frontmatter optional).
#[must_use]
pub fn skills_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => {
            let config = platform::config_dir().ok()?;
            Some(config.join("crush").join("skills"))
        }
        Scope::Project(root) => Some(root.join(".crush").join("skills")),
        Scope::Custom(path) => Some(path.join("skills")),
    }
}

/// Returns the rules directory for the given scope.
///
/// Crush stores rules files at:
/// - **Global**: `~/.config/crush/`
/// - **Project**: Project root directory
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok(),
        Scope::Project(root) => Some(root.clone()),
        Scope::Custom(path) => Some(path.clone()),
    }
}

/// Checks if Crush is installed on this system.
///
/// Currently checks if the global config directory exists.
pub fn is_installed() -> bool {
    global_config_dir().map(|p| p.exists()).unwrap_or(false)
}

/// Parses a single MCP server from Crush's native JSON format.
///
/// Crush uses the same MCP format as OpenCode with "type" field
/// specifying the transport: "stdio", "http", or "sse".
///
/// # Arguments
/// * `value` - The JSON value representing the server config
///
/// # Errors
/// Returns an error if the JSON is malformed or missing required fields.
pub(crate) fn parse_mcp_server(value: &serde_json::Value) -> Result<McpServer> {
    let config = ParseConfig::CRUSH;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.into(),
            reason: "Server config must be an object".into(),
        })?;

    let server_type =
        obj.get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: config.harness_name.into(),
                reason: "Missing 'type' field".into(),
            })?;

    match server_type {
        "stdio" => mcp_parse::parse_stdio_server(obj, &config),
        "http" => mcp_parse::parse_http_server(obj, &config),
        "sse" => mcp_parse::parse_sse_server(obj, &config),
        _ => Err(Error::UnsupportedMcpConfig {
            harness: config.harness_name.into(),
            reason: format!("Unknown server type: {}", server_type),
        }),
    }
}

/// Parses all MCP servers from a Crush config JSON.
///
/// Crush uses "mcp" as the root key (like OpenCode).
///
/// # Arguments
/// * `config` - The full config JSON (expects mcp key)
///
/// # Errors
/// Returns an error if the JSON is malformed.
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    mcp_parse::parse_servers_from_key(config, "mcp", &ParseConfig::CRUSH, parse_mcp_server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EnvValue;
    use serde_json::json;

    #[test]
    fn global_config_dir_is_absolute() {
        // Skip if config dir cannot be determined (CI environments)
        if platform::config_dir().is_err() {
            return;
        }

        let result = global_config_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with("crush"));
    }

    #[test]
    fn project_config_dir_is_relative_to_root() {
        let root = PathBuf::from("/some/project");
        let config = project_config_dir(&root);
        assert_eq!(config, PathBuf::from("/some/project/.crush"));
    }

    #[test]
    fn commands_dir_returns_config_dir() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = commands_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("crush"));
    }

    #[test]
    fn skills_dir_global_returns_crush_skills() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = skills_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with("crush/skills"));
    }

    #[test]
    fn skills_dir_project_returns_dot_crush_skills() {
        let root = PathBuf::from("/some/project");
        let result = skills_dir(&Scope::Project(root));
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/some/project/.crush/skills")
        );
    }

    #[test]
    fn mcp_dir_returns_config_dir() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = mcp_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("crush"));
    }

    #[test]
    fn rules_dir_global_returns_config() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = rules_dir(&Scope::Global);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("crush"));
    }

    #[test]
    fn rules_dir_project_returns_root() {
        let root = PathBuf::from("/some/project");
        let result = rules_dir(&Scope::Project(root.clone()));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), root);
    }

    #[test]
    fn parse_stdio_server_basic() {
        let json = json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server"]
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert_eq!(server.command, "npx");
            assert_eq!(server.args, vec!["-y", "@modelcontextprotocol/server"]);
            assert!(server.enabled);
            assert!(server.env.is_empty());
            assert_eq!(server.timeout_ms, None);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_with_env() {
        let json = json!({
            "type": "stdio",
            "command": "node",
            "args": ["server.js"],
            "env": {
                "API_KEY": "secret123",
                "DEBUG": "true"
            },
            "timeout_ms": 30000
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert_eq!(server.command, "node");
            assert_eq!(server.args, vec!["server.js"]);
            assert_eq!(server.env.len(), 2);
            assert_eq!(
                server.env.get("API_KEY"),
                Some(&EnvValue::plain("secret123"))
            );
            assert_eq!(server.env.get("DEBUG"), Some(&EnvValue::plain("true")));
            assert_eq!(server.timeout_ms, Some(30000));
            assert!(server.enabled);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_disabled() {
        let json = json!({
            "type": "stdio",
            "command": "test",
            "disabled": true
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert!(!server.enabled);
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

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Http(server) = result {
            assert_eq!(server.url, "https://api.example.com/mcp");
            assert!(server.enabled);
            assert!(server.headers.is_empty());
            assert!(server.oauth.is_none());
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
                "Authorization": "Bearer token",
                "X-Custom": "value"
            }
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Http(server) = result {
            assert_eq!(server.url, "https://api.example.com/mcp");
            assert_eq!(server.headers.len(), 2);
            assert_eq!(
                server.headers.get("Authorization"),
                Some(&EnvValue::plain("Bearer token"))
            );
            assert_eq!(
                server.headers.get("X-Custom"),
                Some(&EnvValue::plain("value"))
            );
        } else {
            panic!("Expected Http variant");
        }
    }

    #[test]
    fn parse_sse_server_basic() {
        let json = json!({
            "type": "sse",
            "url": "https://example.com/sse",
            "timeout_ms": 45000
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Sse(server) = result {
            assert_eq!(server.url, "https://example.com/sse");
            assert_eq!(server.timeout_ms, Some(45000));
            assert!(server.enabled);
            assert!(server.headers.is_empty());
        } else {
            panic!("Expected Sse variant");
        }
    }

    #[test]
    fn parse_mcp_server_missing_type() {
        let json = json!({
            "command": "test"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_unknown_type() {
        let json = json!({
            "type": "unknown_type",
            "command": "test"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stdio_server_missing_command() {
        let json = json!({
            "type": "stdio",
            "args": ["test"]
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_http_server_missing_url() {
        let json = json!({
            "type": "http"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_sse_server_missing_url() {
        let json = json!({
            "type": "sse"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_basic() {
        let config = json!({
            "mcp": {
                "server1": {
                    "type": "stdio",
                    "command": "npx",
                    "args": ["-y", "server1"]
                },
                "server2": {
                    "type": "sse",
                    "url": "https://example.com/sse"
                }
            }
        });

        let result = parse_mcp_servers(&config).unwrap();
        assert_eq!(result.len(), 2);

        let names: Vec<&str> = result.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"server1"));
        assert!(names.contains(&"server2"));
    }

    #[test]
    fn parse_mcp_servers_errors_on_invalid() {
        let config = json!({
            "mcp": {
                "valid": {
                    "type": "stdio",
                    "command": "test"
                },
                "invalid": {
                    "type": "stdio"
                }
            }
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_missing_mcp_key() {
        let config = json!({
            "other_key": {}
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_empty_mcp() {
        let config = json!({
            "mcp": {}
        });

        let result = parse_mcp_servers(&config).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn parse_stdio_server_without_args() {
        let json = json!({
            "type": "stdio",
            "command": "test"
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert_eq!(server.command, "test");
            assert!(server.args.is_empty());
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_without_env() {
        let json = json!({
            "type": "stdio",
            "command": "test"
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert!(server.env.is_empty());
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_non_string_args_fails() {
        let json = json!({
            "type": "stdio",
            "command": "test",
            "args": ["-y", 123]
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stdio_server_args_not_array_fails() {
        let json = json!({
            "type": "stdio",
            "command": "test",
            "args": "not-an-array"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stdio_server_env_not_object_fails() {
        let json = json!({
            "type": "stdio",
            "command": "test",
            "env": "not-an-object"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stdio_server_non_string_env_value_fails() {
        let json = json!({
            "type": "stdio",
            "command": "test",
            "env": {
                "KEY": 123
            }
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_http_server_headers_not_object_fails() {
        let json = json!({
            "type": "http",
            "url": "https://example.com",
            "headers": "not-an-object"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_http_server_non_string_header_value_fails() {
        let json = json!({
            "type": "http",
            "url": "https://example.com",
            "headers": {
                "Authorization": 123
            }
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }
}
