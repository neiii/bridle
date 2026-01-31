//! OpenCode harness implementation.
//!
//! OpenCode stores its configuration in:
//! - **Global**: `~/.config/opencode/`
//! - **Project**: `.opencode/` in project root

use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::mcp::{HttpMcpServer, McpServer, OAuthConfig, StdioMcpServer};
use crate::platform;
use crate::types::{EnvValue, HarnessKind, Scope};

/// Returns the global OpenCode configuration directory.
///
/// Returns `~/.config/opencode/` on all platforms.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn global_config_dir() -> Result<PathBuf> {
    Ok(platform::config_dir()?.join("opencode"))
}

/// Returns the project-local OpenCode configuration directory.
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
#[must_use]
pub fn project_config_dir(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".opencode")
}

/// Returns the commands directory for the given scope.
///
/// - **Global**: `~/.config/opencode/command/`
/// - **Project**: `.opencode/command/`
pub fn commands_dir(scope: &Scope) -> Result<PathBuf> {
    match scope {
        Scope::Global => Ok(global_config_dir()?.join("command")),
        Scope::Project(root) => Ok(project_config_dir(root).join("command")),
        Scope::Custom(path) => Ok(path.join("command")),
    }
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
/// OpenCode stores MCP configuration in `opencode.json` under the `mcp` key,
/// NOT in a separate directory. The `plugin/` directory is for JS/TS plugins only.
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
}

/// Returns the skills directory for the given scope.
///
/// - **Global**: `~/.config/opencode/skill/`
/// - **Project**: `.opencode/skill/`
#[must_use]
pub fn skills_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok().map(|p| p.join("skill")),
        Scope::Project(root) => Some(project_config_dir(root).join("skill")),
        Scope::Custom(path) => Some(path.join("skill")),
    }
}

/// Returns the rules directory for the given scope.
///
/// OpenCode stores rules files (`AGENTS.md`) at:
/// - **Global**: None (no global rules)
/// - **Project**: Project root directory
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => None,
        Scope::Project(root) => Some(root.clone()),
        Scope::Custom(path) => Some(path.clone()),
    }
}

/// Checks if OpenCode is installed on this system.
///
/// Currently checks if the global config directory exists.
pub fn is_installed() -> bool {
    global_config_dir().map(|p| p.exists()).unwrap_or(false)
}

/// Parses a single MCP server from OpenCode's native JSON format.
///
/// # Arguments
///
/// * `value` - The JSON value representing the server config
///
/// # Errors
///
/// Returns an error if the JSON is malformed or missing required fields.
#[allow(dead_code)] // Internal utility for future MCP config reading
pub(crate) fn parse_mcp_server(value: &serde_json::Value) -> Result<McpServer> {
    let obj = value
        .as_object()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: "OpenCode".into(),
            reason: "Server config must be an object".into(),
        })?;

    let server_type =
        obj.get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: "OpenCode".into(),
                reason: "Missing 'type' field".into(),
            })?;

    match server_type {
        "local" => parse_local_server(obj),
        "remote" => parse_remote_server(obj),
        other => Err(Error::UnsupportedMcpConfig {
            harness: "OpenCode".into(),
            reason: format!("Unknown server type: {other}"),
        }),
    }
}

/// Parses all MCP servers from an OpenCode config JSON.
///
/// # Arguments
///
/// * `config` - The full config JSON (expects mcp key)
///
/// # Errors
///
/// Returns an error if the JSON is malformed.
#[allow(dead_code)] // Internal utility for future MCP config reading
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    let mcp = config
        .get("mcp")
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: "OpenCode".into(),
            reason: "Missing 'mcp' key in config".into(),
        })?;

    let mcp_obj = mcp.as_object().ok_or_else(|| Error::UnsupportedMcpConfig {
        harness: "OpenCode".into(),
        reason: "'mcp' must be an object".into(),
    })?;

    let mut servers = Vec::new();
    for (name, server_value) in mcp_obj {
        let server = parse_mcp_server(server_value).map_err(|e| Error::UnsupportedMcpConfig {
            harness: "OpenCode".into(),
            reason: format!("server '{}': {}", name, e),
        })?;

        servers.push((name.clone(), server));
    }

    Ok(servers)
}

#[allow(dead_code)] // Internal utility for future MCP config reading
fn parse_local_server(obj: &serde_json::Map<String, serde_json::Value>) -> Result<McpServer> {
    // Parse command array: first element is command, rest are args
    let command_array = obj
        .get("command")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: "OpenCode".into(),
            reason: "Missing or invalid 'command' field".into(),
        })?;

    if command_array.is_empty() {
        return Err(Error::UnsupportedMcpConfig {
            harness: "OpenCode".into(),
            reason: "Command array must not be empty".into(),
        });
    }

    let command = command_array[0]
        .as_str()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: "OpenCode".into(),
            reason: "Command must be a string".into(),
        })?
        .to_string();

    let args: Vec<String> = command_array[1..]
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| Error::UnsupportedMcpConfig {
                    harness: "OpenCode".into(),
                    reason: "Command arguments must be strings".into(),
                })
                .map(String::from)
        })
        .collect::<Result<Vec<_>>>()?;

    // Parse environment variables
    let mut env = HashMap::new();
    if let Some(environment) = obj.get("environment") {
        let env_obj = environment
            .as_object()
            .ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: "OpenCode".into(),
                reason: "'environment' must be an object".into(),
            })?;

        for (key, value) in env_obj {
            let value_str = value.as_str().ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: "OpenCode".into(),
                reason: "Environment variable values must be strings".into(),
            })?;
            env.insert(
                key.clone(),
                EnvValue::from_native(value_str, HarnessKind::OpenCode),
            );
        }
    }

    // Parse enabled flag (defaults to true)
    let enabled = obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

    // Parse timeout in milliseconds
    let timeout_ms = obj.get("timeout").and_then(|v| v.as_u64());

    Ok(McpServer::Stdio(StdioMcpServer {
        command,
        args,
        env,
        cwd: None,
        enabled,
        timeout_ms,
    }))
}

#[allow(dead_code)] // Internal utility for future MCP config reading
fn parse_remote_server(obj: &serde_json::Map<String, serde_json::Value>) -> Result<McpServer> {
    // Parse URL
    let url = obj
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: "OpenCode".into(),
            reason: "Missing or invalid 'url' field".into(),
        })?
        .to_string();

    // Parse headers
    let mut headers = HashMap::new();
    if let Some(headers_value) = obj.get("headers") {
        let headers_obj = headers_value
            .as_object()
            .ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: "OpenCode".into(),
                reason: "'headers' must be an object".into(),
            })?;

        for (key, value) in headers_obj {
            let value_str = value.as_str().ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: "OpenCode".into(),
                reason: "Header values must be strings".into(),
            })?;
            headers.insert(
                key.clone(),
                EnvValue::from_native(value_str, HarnessKind::OpenCode),
            );
        }
    }

    let oauth = if let Some(oauth_value) = obj.get("oauth") {
        let oauth_obj = oauth_value
            .as_object()
            .ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: "OpenCode".into(),
                reason: "'oauth' must be an object".into(),
            })?;

        let client_id = if let Some(v) = oauth_obj.get("client_id") {
            Some(
                v.as_str()
                    .ok_or_else(|| Error::UnsupportedMcpConfig {
                        harness: "OpenCode".into(),
                        reason: "oauth.client_id must be a string".into(),
                    })?
                    .to_string(),
            )
        } else {
            None
        };

        let client_secret = if let Some(v) = oauth_obj.get("client_secret") {
            Some(EnvValue::from_native(
                v.as_str().ok_or_else(|| Error::UnsupportedMcpConfig {
                    harness: "OpenCode".into(),
                    reason: "oauth.client_secret must be a string".into(),
                })?,
                HarnessKind::OpenCode,
            ))
        } else {
            None
        };

        let scope = if let Some(v) = oauth_obj.get("scope") {
            Some(
                v.as_str()
                    .ok_or_else(|| Error::UnsupportedMcpConfig {
                        harness: "OpenCode".into(),
                        reason: "oauth.scope must be a string".into(),
                    })?
                    .to_string(),
            )
        } else {
            None
        };

        Some(OAuthConfig {
            client_id,
            client_secret,
            scope,
        })
    } else {
        None
    };

    // Parse enabled flag (defaults to true)
    let enabled = obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

    // Parse timeout in milliseconds
    let timeout_ms = obj.get("timeout").and_then(|v| v.as_u64());

    Ok(McpServer::Http(HttpMcpServer {
        url,
        headers,
        oauth,
        enabled,
        timeout_ms,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(path.ends_with("opencode"));
    }

    #[test]
    fn project_config_dir_is_relative_to_root() {
        let root = PathBuf::from("/some/project");
        let config = project_config_dir(&root);
        assert_eq!(config, PathBuf::from("/some/project/.opencode"));
    }

    #[test]
    fn commands_dir_global() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = commands_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("command"));
    }

    #[test]
    fn commands_dir_project() {
        let root = PathBuf::from("/some/project");
        let result = commands_dir(&Scope::Project(root));
        assert!(result.is_ok());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.opencode/command"));
    }

    #[test]
    fn skills_dir_global() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = skills_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with("skill"));
    }

    #[test]
    fn skills_dir_project() {
        let root = PathBuf::from("/some/project");
        let result = skills_dir(&Scope::Project(root));
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.opencode/skill"));
    }

    #[test]
    fn mcp_dir_global() {
        if platform::config_dir().is_err() {
            return;
        }

        let result = mcp_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("opencode"));
    }

    #[test]
    fn rules_dir_global_returns_none() {
        assert!(rules_dir(&Scope::Global).is_none());
    }

    #[test]
    fn rules_dir_project_returns_root() {
        let root = PathBuf::from("/some/project");
        let result = rules_dir(&Scope::Project(root.clone()));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), root);
    }

    #[test]
    fn parse_local_server_basic() {
        let config = json!({
            "type": "local",
            "command": ["npx", "-y", "@modelcontextprotocol/server"],
            "enabled": true
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Stdio(s) = server {
            assert_eq!(s.command, "npx");
            assert_eq!(s.args, vec!["-y", "@modelcontextprotocol/server"]);
            assert!(s.env.is_empty());
            assert!(s.enabled);
            assert_eq!(s.timeout_ms, None);
        } else {
            panic!("Expected Stdio server");
        }
    }

    #[test]
    fn parse_local_server_with_environment() {
        let config = json!({
            "type": "local",
            "command": ["node", "server.js"],
            "environment": {
                "API_KEY": "{env:MY_API_KEY}",
                "DEBUG": "true"
            },
            "enabled": true
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Stdio(s) = server {
            assert_eq!(s.command, "node");
            assert_eq!(s.args, vec!["server.js"]);
            assert_eq!(s.env.len(), 2);
            assert_eq!(s.env.get("API_KEY"), Some(&EnvValue::env("MY_API_KEY")));
            assert_eq!(s.env.get("DEBUG"), Some(&EnvValue::plain("true")));
        } else {
            panic!("Expected Stdio server");
        }
    }

    #[test]
    fn parse_local_server_with_timeout() {
        let config = json!({
            "type": "local",
            "command": ["test"],
            "timeout": 30000,
            "enabled": true
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Stdio(s) = server {
            assert_eq!(s.timeout_ms, Some(30000));
        } else {
            panic!("Expected Stdio server");
        }
    }

    #[test]
    fn parse_local_server_disabled() {
        let config = json!({
            "type": "local",
            "command": ["test"],
            "enabled": false
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Stdio(s) = server {
            assert!(!s.enabled);
        } else {
            panic!("Expected Stdio server");
        }
    }

    #[test]
    fn parse_local_server_enabled_defaults_true() {
        let config = json!({
            "type": "local",
            "command": ["test"]
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Stdio(s) = server {
            assert!(s.enabled);
        } else {
            panic!("Expected Stdio server");
        }
    }

    #[test]
    fn parse_remote_server_basic() {
        let config = json!({
            "type": "remote",
            "url": "https://api.example.com/mcp",
            "enabled": true
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Http(s) = server {
            assert_eq!(s.url, "https://api.example.com/mcp");
            assert!(s.headers.is_empty());
            assert!(s.oauth.is_none());
            assert!(s.enabled);
            assert_eq!(s.timeout_ms, None);
        } else {
            panic!("Expected Http server");
        }
    }

    #[test]
    fn parse_remote_server_with_headers() {
        let config = json!({
            "type": "remote",
            "url": "https://api.example.com/mcp",
            "headers": {
                "Authorization": "{env:TOKEN}",
                "X-Custom": "value"
            },
            "enabled": true
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Http(s) = server {
            assert_eq!(s.headers.len(), 2);
            assert_eq!(
                s.headers.get("Authorization"),
                Some(&EnvValue::env("TOKEN"))
            );
            assert_eq!(s.headers.get("X-Custom"), Some(&EnvValue::plain("value")));
        } else {
            panic!("Expected Http server");
        }
    }

    #[test]
    fn parse_remote_server_with_oauth() {
        let config = json!({
            "type": "remote",
            "url": "https://api.example.com/mcp",
            "oauth": {
                "client_id": "my-app",
                "client_secret": "{env:OAUTH_SECRET}",
                "scope": "read write"
            },
            "enabled": true
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Http(s) = server {
            assert!(s.oauth.is_some());
            let oauth = s.oauth.unwrap();
            assert_eq!(oauth.client_id, Some("my-app".to_string()));
            assert_eq!(oauth.client_secret, Some(EnvValue::env("OAUTH_SECRET")));
            assert_eq!(oauth.scope, Some("read write".to_string()));
        } else {
            panic!("Expected Http server");
        }
    }

    #[test]
    fn parse_remote_server_with_timeout() {
        let config = json!({
            "type": "remote",
            "url": "https://api.example.com/mcp",
            "timeout": 60000,
            "enabled": true
        });

        let server = parse_mcp_server(&config).unwrap();

        if let McpServer::Http(s) = server {
            assert_eq!(s.timeout_ms, Some(60000));
        } else {
            panic!("Expected Http server");
        }
    }

    #[test]
    fn parse_mcp_server_missing_type() {
        let config = json!({
            "command": ["test"]
        });

        let result = parse_mcp_server(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_invalid_type() {
        let config = json!({
            "type": "invalid",
            "command": ["test"]
        });

        let result = parse_mcp_server(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_local_server_missing_command() {
        let config = json!({
            "type": "local",
            "enabled": true
        });

        let result = parse_mcp_server(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_local_server_empty_command() {
        let config = json!({
            "type": "local",
            "command": [],
            "enabled": true
        });

        let result = parse_mcp_server(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_remote_server_missing_url() {
        let config = json!({
            "type": "remote",
            "enabled": true
        });

        let result = parse_mcp_server(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_basic() {
        let config = json!({
            "mcp": {
                "server1": {
                    "type": "local",
                    "command": ["node", "server1.js"],
                    "enabled": true
                },
                "server2": {
                    "type": "remote",
                    "url": "https://example.com",
                    "enabled": true
                }
            }
        });

        let servers = parse_mcp_servers(&config).unwrap();
        assert_eq!(servers.len(), 2);

        let names: Vec<&str> = servers.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"server1"));
        assert!(names.contains(&"server2"));
    }

    #[test]
    fn parse_mcp_servers_includes_disabled() {
        let config = json!({
            "mcp": {
                "enabled-server": {
                    "type": "local",
                    "command": ["test"],
                    "enabled": true
                },
                "disabled-server": {
                    "type": "local",
                    "command": ["test"],
                    "enabled": false
                }
            }
        });

        let servers = parse_mcp_servers(&config).unwrap();
        assert_eq!(servers.len(), 2);

        let names: Vec<&str> = servers.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"enabled-server"));
        assert!(names.contains(&"disabled-server"));
    }

    #[test]
    fn parse_mcp_servers_errors_on_invalid() {
        let config = json!({
            "mcp": {
                "valid-server": {
                    "type": "local",
                    "command": ["test"],
                    "enabled": true
                },
                "invalid-server": {
                    "type": "invalid"
                }
            }
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_missing_mcp_key() {
        let config = json!({
            "other": {}
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_empty() {
        let config = json!({
            "mcp": {}
        });

        let servers = parse_mcp_servers(&config).unwrap();
        assert!(servers.is_empty());
    }

    #[test]
    fn parse_remote_server_oauth_invalid_client_id_type() {
        let config = json!({
            "type": "remote",
            "url": "https://api.example.com/mcp",
            "oauth": {
                "client_id": 123
            },
            "enabled": true
        });

        let result = parse_mcp_server(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_remote_server_oauth_invalid_client_secret_type() {
        let config = json!({
            "type": "remote",
            "url": "https://api.example.com/mcp",
            "oauth": {
                "client_secret": 123
            },
            "enabled": true
        });

        let result = parse_mcp_server(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_remote_server_oauth_invalid_scope_type() {
        let config = json!({
            "type": "remote",
            "url": "https://api.example.com/mcp",
            "oauth": {
                "scope": 123
            },
            "enabled": true
        });

        let result = parse_mcp_server(&config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_full_example() {
        let config = json!({
            "mcp": {
                "filesystem": {
                    "type": "local",
                    "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem"],
                    "environment": {
                        "ALLOWED_PATH": "/tmp"
                    },
                    "enabled": true,
                    "timeout": 30000
                },
                "api-server": {
                    "type": "remote",
                    "url": "https://api.example.com/mcp",
                    "headers": {
                        "Authorization": "{env:API_TOKEN}"
                    },
                    "oauth": {
                        "client_id": "my-client",
                        "client_secret": "{env:OAUTH_SECRET}",
                        "scope": "read write"
                    },
                    "enabled": true,
                    "timeout": 60000
                },
                "disabled-server": {
                    "type": "local",
                    "command": ["test"],
                    "enabled": false
                }
            }
        });

        let servers = parse_mcp_servers(&config).unwrap();
        assert_eq!(servers.len(), 3);

        let filesystem = servers
            .iter()
            .find(|(n, _)| n == "filesystem")
            .map(|(_, s)| s);
        assert!(filesystem.is_some());

        if let Some(McpServer::Stdio(s)) = filesystem {
            assert_eq!(s.command, "npx");
            assert_eq!(
                s.args,
                vec!["-y", "@modelcontextprotocol/server-filesystem"]
            );
            assert_eq!(s.timeout_ms, Some(30000));
        } else {
            panic!("Expected Stdio server for filesystem");
        }

        let api_server = servers
            .iter()
            .find(|(n, _)| n == "api-server")
            .map(|(_, s)| s);
        assert!(api_server.is_some());

        if let Some(McpServer::Http(s)) = api_server {
            assert_eq!(s.url, "https://api.example.com/mcp");
            assert_eq!(s.timeout_ms, Some(60000));
            assert!(s.oauth.is_some());
        } else {
            panic!("Expected Http server for api-server");
        }
    }
}
