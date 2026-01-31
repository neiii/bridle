//! Goose harness implementation.
//!
//! Goose stores its configuration in:
//! - **Global**: `~/.config/goose/`
//! - **Project**: `.goose/` in project root (if exists)

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::mcp::McpServer;
use crate::platform;
use crate::types::Scope;

use super::mcp_parse::{self, ParseConfig};

/// Returns the global Goose configuration directory.
///
/// Returns `~/.config/goose/` on all platforms.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn global_config_dir() -> Result<PathBuf> {
    Ok(platform::config_dir()?.join("goose"))
}

/// Returns the project-local Goose configuration directory.
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
#[must_use]
pub fn project_config_dir(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".goose")
}

/// Returns the commands directory for the given scope.
///
/// Goose does not have a dedicated commands directory, so this
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
/// Goose stores MCP configuration in the base config directory.
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
}

/// Returns the skills directory path for Goose.
///
/// Goose stores skills in:
/// - Global: `~/.config/agents/skills/`
/// - Project: `.agents/skills/`
///
/// Skills use the same SKILL.md format with YAML frontmatter.
#[must_use]
pub fn skills_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => {
            let config = platform::config_dir().ok()?;
            Some(config.join("agents").join("skills"))
        }
        Scope::Project(root) => Some(root.join(".agents").join("skills")),
        Scope::Custom(path) => Some(path.join("skills")),
    }
}

/// Returns the rules directory for the given scope.
///
/// Goose stores rules files (`.goosehints`, `AGENTS.md`) at:
/// - **Global**: `~/.config/goose/`
/// - **Project**: Project root directory
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok(),
        Scope::Project(root) => Some(root.clone()),
        Scope::Custom(path) => Some(path.clone()),
    }
}

/// Checks if Goose is installed on this system.
///
/// Currently checks if the global config directory exists.
pub fn is_installed() -> bool {
    global_config_dir().map(|p| p.exists()).unwrap_or(false)
}

/// Parses a single MCP server from Goose's native JSON format.
///
/// # Arguments
/// * `value` - The JSON value representing the server config
///
/// # Errors
/// Returns an error if the JSON is malformed or missing required fields.
#[allow(dead_code)] // Internal utility for future MCP config reading
pub(crate) fn parse_mcp_server(value: &serde_json::Value) -> Result<McpServer> {
    let config = ParseConfig::GOOSE;
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
        "sse" => mcp_parse::parse_sse_server(obj, &config),
        "streamable_http" => mcp_parse::parse_http_server(obj, &config),
        _ => Err(Error::UnsupportedMcpConfig {
            harness: config.harness_name.into(),
            reason: format!("Unknown server type: {}", server_type),
        }),
    }
}

/// Parses all MCP servers from a Goose config JSON.
///
/// # Arguments
/// * `config` - The full config JSON (expects extensions key)
///
/// # Errors
/// Returns an error if the JSON is malformed.
#[allow(dead_code)] // Internal utility for future MCP config reading
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    mcp_parse::parse_servers_from_key(config, "extensions", &ParseConfig::GOOSE, parse_mcp_server)
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
        assert!(path.ends_with("goose"));
    }

    #[test]
    fn project_config_dir_is_relative_to_root() {
        let root = PathBuf::from("/some/project");
        let config = project_config_dir(&root);
        assert_eq!(config, PathBuf::from("/some/project/.goose"));
    }

    #[test]
    fn commands_dir_returns_config_dir() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = commands_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("goose"));
    }

    #[test]
    fn skills_dir_global_returns_agents_skills() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = skills_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with("agents/skills"));
    }

    #[test]
    fn skills_dir_project_returns_dot_agents_skills() {
        let root = PathBuf::from("/some/project");
        let result = skills_dir(&Scope::Project(root));
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/some/project/.agents/skills")
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
        assert!(path.ends_with("goose"));
    }

    #[test]
    fn rules_dir_global_returns_config() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = rules_dir(&Scope::Global);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("goose"));
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
            "cmd": "npx",
            "args": ["-y", "@modelcontextprotocol/server"],
            "enabled": true
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
    fn parse_stdio_server_with_envs() {
        let json = json!({
            "type": "stdio",
            "cmd": "node",
            "args": ["server.js"],
            "envs": {
                "API_KEY": "secret123",
                "DEBUG": "true"
            },
            "timeout": 30,
            "enabled": true
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
    fn parse_stdio_server_timeout_converts_to_milliseconds() {
        let json = json!({
            "type": "stdio",
            "cmd": "test",
            "timeout": 45
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert_eq!(server.timeout_ms, Some(45000));
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_defaults_enabled_to_true() {
        let json = json!({
            "type": "stdio",
            "cmd": "test"
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert!(server.enabled);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_disabled() {
        let json = json!({
            "type": "stdio",
            "cmd": "test",
            "enabled": false
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert!(!server.enabled);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_sse_server_basic() {
        let json = json!({
            "type": "sse",
            "uri": "https://example.com/sse",
            "timeout": 45,
            "enabled": true
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
    fn parse_http_server_basic() {
        let json = json!({
            "type": "streamable_http",
            "uri": "https://api.example.com/mcp",
            "timeout": 60,
            "enabled": true
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Http(server) = result {
            assert_eq!(server.url, "https://api.example.com/mcp");
            assert_eq!(server.timeout_ms, Some(60000));
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
            "type": "streamable_http",
            "uri": "https://api.example.com/mcp",
            "headers": {
                "Authorization": "Bearer token",
                "X-Custom": "value"
            },
            "enabled": true
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
    fn parse_mcp_server_missing_type() {
        let json = json!({
            "cmd": "test"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_unknown_type() {
        let json = json!({
            "type": "unknown_type",
            "cmd": "test"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stdio_server_missing_cmd() {
        let json = json!({
            "type": "stdio",
            "args": ["test"]
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_sse_server_missing_uri() {
        let json = json!({
            "type": "sse",
            "enabled": true
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_http_server_missing_uri() {
        let json = json!({
            "type": "streamable_http",
            "enabled": true
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_basic() {
        let config = json!({
            "extensions": {
                "server1": {
                    "type": "stdio",
                    "cmd": "npx",
                    "args": ["-y", "server1"]
                },
                "server2": {
                    "type": "sse",
                    "uri": "https://example.com/sse"
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
            "extensions": {
                "valid": {
                    "type": "stdio",
                    "cmd": "test"
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
    fn parse_mcp_servers_missing_extensions() {
        let config = json!({
            "other_key": {}
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_empty_extensions() {
        let config = json!({
            "extensions": {}
        });

        let result = parse_mcp_servers(&config).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn parse_stdio_server_without_args() {
        let json = json!({
            "type": "stdio",
            "cmd": "test"
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
    fn parse_stdio_server_without_envs() {
        let json = json!({
            "type": "stdio",
            "cmd": "test"
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
            "cmd": "test",
            "args": ["-y", 123]
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stdio_server_args_not_array_fails() {
        let json = json!({
            "type": "stdio",
            "cmd": "test",
            "args": "not-an-array"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stdio_server_envs_not_object_fails() {
        let json = json!({
            "type": "stdio",
            "cmd": "test",
            "envs": "not-an-object"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stdio_server_non_string_env_value_fails() {
        let json = json!({
            "type": "stdio",
            "cmd": "test",
            "envs": {
                "KEY": 123
            }
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_http_server_headers_not_object_fails() {
        let json = json!({
            "type": "streamable_http",
            "uri": "https://example.com",
            "headers": "not-an-object"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_http_server_non_string_header_value_fails() {
        let json = json!({
            "type": "streamable_http",
            "uri": "https://example.com",
            "headers": {
                "Authorization": 123
            }
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_stdio_with_full_config() {
        let json = json!({
            "type": "stdio",
            "cmd": "node",
            "args": ["server.js", "--port", "3000"],
            "envs": {
                "NODE_ENV": "production",
                "API_KEY": "key123"
            },
            "timeout": 30,
            "enabled": false
        });

        let parsed = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = parsed {
            assert_eq!(server.command, "node");
            assert_eq!(server.args, vec!["server.js", "--port", "3000"]);
            assert_eq!(server.env.len(), 2);
            assert_eq!(server.timeout_ms, Some(30000));
            assert!(!server.enabled);
        } else {
            panic!("Expected Stdio variant");
        }
    }
}
