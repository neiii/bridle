//! AMP Code harness implementation.
//!
//! AMP Code stores its configuration in:
//! - **Global**: `~/.config/amp/`
//! - **Project**: Not supported (AMP has no project-scoped config directory)
//!
//! Note: Skills are shared with Goose at `~/.config/agents/skills/`.

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::mcp::McpServer;
use crate::platform;
use crate::types::Scope;

use super::mcp_parse::{self, ParseConfig};

/// Returns the global AMP Code configuration directory.
///
/// Returns `~/.config/amp/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn global_config_dir() -> Result<PathBuf> {
    Ok(platform::config_dir()?.join("amp"))
}

/// Returns the config directory for the given scope.
///
/// - **Global**: `~/.config/amp/`
/// - **Project**: Returns `UnsupportedScope` error (AMP has no project config)
///
/// # Errors
///
/// Returns `Error::UnsupportedScope` for project scope.
pub fn config_dir(scope: &Scope) -> Result<PathBuf> {
    match scope {
        Scope::Global => global_config_dir(),
        Scope::Project(_) => Err(Error::UnsupportedScope {
            harness: "AMP Code".to_string(),
            scope: "project".to_string(),
        }),
        Scope::Custom(path) => Ok(path.clone()),
    }
}

/// Returns the commands directory for the given scope.
///
/// - **Global**: `~/.config/amp/commands/`
/// - **Project**: `.agents/commands/`
pub fn commands_dir(scope: &Scope) -> Result<PathBuf> {
    match scope {
        Scope::Global => Ok(global_config_dir()?.join("commands")),
        Scope::Project(root) => Ok(root.join(".agents").join("commands")),
        Scope::Custom(path) => Ok(path.join("commands")),
    }
}

/// Returns the MCP configuration directory for the given scope.
///
/// AMP stores MCP configuration in `settings.json` within the config directory.
///
/// - **Global**: `~/.config/amp/`
/// - **Project**: Returns `UnsupportedScope` error
///
/// # Errors
///
/// Returns `Error::UnsupportedScope` for project scope.
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
}

/// Returns the skills directory for the given scope.
///
/// AMP shares the skills directory with Goose:
/// - **Global**: `~/.config/agents/skills/`
/// - **Project**: `.agents/skills/`
#[must_use]
pub fn skills_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => platform::config_dir()
            .ok()
            .map(|p| p.join("agents").join("skills")),
        Scope::Project(root) => Some(root.join(".agents").join("skills")),
        Scope::Custom(path) => Some(path.join("skills")),
    }
}

/// Returns the rules directory for the given scope.
///
/// AMP stores rules files (`AGENTS.md`) at:
/// - **Global**: `~/.config/amp/`
/// - **Project**: Project root directory
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok(),
        Scope::Project(root) => Some(root.clone()),
        Scope::Custom(path) => Some(path.clone()),
    }
}

/// Checks if AMP Code is installed on this system.
///
/// Checks if the `amp` binary is available in PATH.
pub fn is_installed() -> bool {
    which::which("amp").is_ok()
}

/// Parses a single MCP server from AMP's native JSON format.
///
/// AMP uses the same format as Claude Code:
/// - `command`: string (required for stdio)
/// - `args`: array of strings
/// - `env`: object with `${VAR}` syntax for environment references
/// - `type`: "stdio" | "sse" | "http"
/// - `url`: string (required for sse/http)
/// - `headers`: object
///
/// # Errors
///
/// Returns an error if the JSON is malformed or missing required fields.
#[allow(dead_code)]
pub(crate) fn parse_mcp_server(name: &str, value: &serde_json::Value) -> Result<McpServer> {
    let config = ParseConfig::AMP_CODE;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: "Server configuration must be an object".to_string(),
        })?;

    if let Some(server_type) = obj.get("type").and_then(|v| v.as_str()) {
        match server_type {
            "sse" => mcp_parse::parse_sse_server(obj, &config),
            "http" => mcp_parse::parse_http_server(obj, &config),
            "stdio" => mcp_parse::parse_stdio_server(obj, &config),
            _ => Err(Error::UnsupportedMcpConfig {
                harness: config.harness_name.to_string(),
                reason: format!("Unknown server type: {}", server_type),
            }),
        }
    } else if obj.contains_key("url") && obj.contains_key("command") {
        Err(Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: format!(
                "Server '{}' has both 'command' and 'url' fields - specify 'type' to disambiguate",
                name
            ),
        })
    } else if obj.contains_key("url") {
        mcp_parse::parse_http_server(obj, &config)
    } else if obj.contains_key("command") {
        mcp_parse::parse_stdio_server(obj, &config)
    } else {
        Err(Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: format!(
                "Server '{}' has neither 'command' (stdio) nor 'url' (http) field",
                name
            ),
        })
    }
}

/// Parses all MCP servers from an AMP settings.json config.
///
/// # Arguments
/// * `config` - The full config JSON (expects `amp.mcpServers` key path)
///
/// # Errors
/// Returns an error if the JSON is malformed.
#[allow(dead_code)]
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    // Try literal dotted key first (actual AmpCode format), then nested fallback
    let servers_obj = config
        .get("amp.mcpServers")
        .or_else(|| config.get("amp").and_then(|v| v.get("mcpServers")))
        .and_then(|v| v.as_object());

    let Some(servers_obj) = servers_obj else {
        return Ok(vec![]);
    };

    servers_obj
        .iter()
        .map(|(name, value)| {
            let server = parse_mcp_server(name, value)?;
            Ok((name.clone(), server))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EnvValue;
    use serde_json::json;

    #[test]
    fn global_config_dir_is_absolute() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = global_config_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with("amp"));
    }

    #[test]
    fn config_dir_global() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = config_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("amp"));
    }

    #[test]
    fn config_dir_project_returns_unsupported_scope() {
        let root = PathBuf::from("/some/project");
        let result = config_dir(&Scope::Project(root));
        assert!(result.is_err());

        if let Err(Error::UnsupportedScope { harness, scope }) = result {
            assert_eq!(harness, "AMP Code");
            assert_eq!(scope, "project");
        } else {
            panic!("Expected UnsupportedScope error");
        }
    }

    #[test]
    fn commands_dir_global() {
        if platform::config_dir().is_err() {
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
        assert_eq!(path, PathBuf::from("/some/project/.agents/commands"));
    }

    #[test]
    fn skills_dir_global_shared_with_goose() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = skills_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with("agents/skills"));
    }

    #[test]
    fn skills_dir_project() {
        let root = PathBuf::from("/some/project");
        let result = skills_dir(&Scope::Project(root));
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.agents/skills"));
    }

    #[test]
    fn rules_dir_global() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = rules_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with("amp"));
    }

    #[test]
    fn rules_dir_project_returns_root() {
        let root = PathBuf::from("/some/project");
        let result = rules_dir(&Scope::Project(root.clone()));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), root);
    }

    #[test]
    fn mcp_dir_project_returns_unsupported_scope() {
        let root = PathBuf::from("/some/project");
        let result = mcp_dir(&Scope::Project(root));
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::UnsupportedScope { .. })));
    }

    #[test]
    fn parse_stdio_server_basic() {
        let json = json!({
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"]
        });

        let result = parse_mcp_server("test", &json);
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

        let result = parse_mcp_server("test", &json);
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
    fn parse_stdio_server_without_args() {
        let json = json!({
            "command": "my-server"
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.command, "my-server");
            assert!(server.args.is_empty());
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_sse_server_basic() {
        let json = json!({
            "type": "sse",
            "url": "https://example.com/sse"
        });

        let result = parse_mcp_server("test", &json);
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
        let json = json!({
            "type": "sse",
            "url": "https://example.com/sse",
            "headers": {
                "Authorization": "${TOKEN}",
                "X-Custom-Header": "value"
            }
        });

        let result = parse_mcp_server("test", &json);
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
        let json = json!({
            "type": "http",
            "url": "https://api.example.com/mcp"
        });

        let result = parse_mcp_server("test", &json);
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

        let result = parse_mcp_server("test", &json);
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
    fn parse_mcp_server_missing_command_fails() {
        let json = json!({
            "args": ["server.js"]
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_missing_url_for_sse_fails() {
        let json = json!({
            "type": "sse"
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_missing_url_for_http_fails() {
        let json = json!({
            "type": "http"
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_unknown_type_fails() {
        let json = json!({
            "type": "unknown",
            "url": "https://example.com"
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_not_object_fails() {
        let json = json!("not an object");

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_full_config() {
        let config = json!({
            "amp": {
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
            "amp": {
                "mcpServers": {}
            }
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_mcp_servers_missing_config_returns_empty() {
        let config = json!({
            "other": "data"
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_mcp_servers_nested_without_mcp_servers_returns_empty() {
        let config = json!({
            "amp": {
                "other": "data"
            }
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parse_mcp_servers_dotted_key_format() {
        let config = json!({
            "amp.mcpServers": {
                "test-server": {
                    "command": "test-cmd",
                    "args": ["--flag"]
                }
            }
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_ok());
        let servers = result.unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].0, "test-server");
    }

    #[test]
    fn parse_env_value_with_dollar_brace_syntax() {
        let json = json!({
            "command": "test",
            "env": {
                "VAR1": "${ENV_VAR}",
                "VAR2": "plain_value"
            }
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.env.get("VAR1"), Some(&EnvValue::env("ENV_VAR")));
            assert_eq!(
                server.env.get("VAR2"),
                Some(&EnvValue::plain("plain_value"))
            );
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_stdio_server_with_explicit_type() {
        let json = json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "server"]
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_ok());

        if let McpServer::Stdio(server) = result.unwrap() {
            assert_eq!(server.command, "npx");
            assert_eq!(server.args, vec!["-y", "server"]);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_mcp_server_non_string_args_fails() {
        let json = json!({
            "command": "test",
            "args": ["-y", 123, "server"]
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_args_not_array_fails() {
        let json = json!({
            "command": "test",
            "args": "not-an-array"
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_env_not_object_fails() {
        let json = json!({
            "command": "test",
            "env": "not-an-object"
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_headers_not_object_fails() {
        let json = json!({
            "type": "sse",
            "url": "https://example.com",
            "headers": "not-an-object"
        });

        let result = parse_mcp_server("test", &json);
        assert!(result.is_err());
    }

    #[test]
    fn infer_stdio_from_command_field() {
        let json = json!({
            "command": "npx",
            "args": ["-y", "some-server"]
        });

        let result = parse_mcp_server("test-stdio", &json).unwrap();
        assert!(matches!(result, McpServer::Stdio(_)));
    }

    #[test]
    fn infer_http_from_url_field() {
        let json = json!({
            "url": "https://example.com/mcp",
            "headers": { "Authorization": "Bearer token" }
        });

        let result = parse_mcp_server("test-http", &json).unwrap();
        assert!(matches!(result, McpServer::Http(_)));
    }

    #[test]
    fn ambiguous_config_with_both_command_and_url_fails() {
        let json = json!({
            "command": "npx",
            "url": "https://example.com"
        });

        let result = parse_mcp_server("ambiguous", &json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("ambiguous"));
        assert!(err.contains("both"));
    }

    #[test]
    fn neither_command_nor_url_fails() {
        let json = json!({
            "env": { "FOO": "bar" }
        });

        let result = parse_mcp_server("incomplete", &json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("incomplete"));
        assert!(err.contains("neither"));
    }
}
