//! Gemini CLI harness implementation.
//!
//! Gemini CLI (Google's AI coding assistant) stores its configuration in:
//! - **Global**: `~/.gemini/`
//! - **Project**: `.gemini/` in project root

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::mcp::McpServer;
use crate::platform;
use crate::types::Scope;

use super::mcp_parse::{self, ParseConfig};

/// Returns the global Gemini CLI configuration directory.
///
/// Returns `~/.gemini/` on all platforms.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined.
pub fn global_config_dir() -> Result<PathBuf> {
    Ok(platform::home_dir()?.join(".gemini"))
}

/// Returns the project-local Gemini CLI configuration directory.
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
#[must_use]
pub fn project_config_dir(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".gemini")
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
/// Gemini CLI stores MCP configuration in the base config directory
/// (settings.json).
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
}

/// Returns the rules directory for the given scope.
///
/// Gemini CLI stores rules files (GEMINI.md) in `~/.gemini/` and within the
/// project tree (root, ancestors, and subdirectories). This returns the
/// primary directory for the scope.
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok(),
        Scope::Project(root) => Some(root.clone()),
        Scope::Custom(path) => Some(path.clone()),
    }
}

/// Checks if Gemini CLI is installed on this system.
///
/// Currently checks if the global config directory exists.
pub fn is_installed() -> bool {
    global_config_dir().map(|p| p.exists()).unwrap_or(false)
}

/// Parses a single MCP server from Gemini CLI's native JSON format.
///
/// Gemini CLI uses `settings.json` with `mcpServers` entries. Each server
/// specifies one of `command` (stdio), `url` (SSE), or `httpUrl` (HTTP).
///
/// # Arguments
/// * `value` - The JSON value representing the server config
///
/// # Errors
/// Returns an error if the JSON is malformed or missing required fields.
pub(crate) fn parse_mcp_server(value: &serde_json::Value) -> Result<McpServer> {
    let config = ParseConfig::GEMINI_CLI;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.into(),
            reason: "Server config must be an object".into(),
        })?;

    if let Some(http_url) = obj.get("httpUrl") {
        let url = http_url
            .as_str()
            .ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: config.harness_name.into(),
                reason: "'httpUrl' must be a string".into(),
            })?
            .to_string();
        let mut normalized = obj.clone();
        normalized.insert("url".to_string(), serde_json::Value::String(url));
        return mcp_parse::parse_http_server(&normalized, &config);
    }

    if obj.get("url").is_some() {
        return mcp_parse::parse_sse_server(obj, &config);
    }

    if obj.get("command").is_some() {
        return mcp_parse::parse_stdio_server(obj, &config);
    }

    Err(Error::UnsupportedMcpConfig {
        harness: config.harness_name.into(),
        reason: "MCP server missing 'command', 'url', or 'httpUrl'".into(),
    })
}

/// Parses all MCP servers from a Gemini CLI config JSON.
///
/// Gemini CLI's MCP configuration is stored under `mcpServers`. If not found,
/// returns an empty vec
/// (no error).
///
/// # Arguments
/// * `config` - The full config JSON
///
/// # Errors
/// Returns an error if the JSON is malformed.
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    // Gemini CLI may not have MCP servers configured - return empty vec if key missing
    if config.get("mcpServers").is_none() {
        return Ok(Vec::new());
    }
    mcp_parse::parse_servers_from_key(
        config,
        "mcpServers",
        &ParseConfig::GEMINI_CLI,
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
        assert!(path.ends_with(".gemini"));
    }

    #[test]
    fn project_config_dir_is_relative_to_root() {
        let root = PathBuf::from("/some/project");
        let config = project_config_dir(&root);
        assert_eq!(config, PathBuf::from("/some/project/.gemini"));
    }

    #[test]
    fn config_dir_global_returns_home_gemini() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = config_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with(".gemini"));
    }

    #[test]
    fn config_dir_project_returns_dot_gemini() {
        let root = PathBuf::from("/some/project");
        let result = config_dir(&Scope::Project(root));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/some/project/.gemini"));
    }

    #[test]
    fn mcp_dir_returns_config_dir() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = mcp_dir(&Scope::Global);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with(".gemini"));
    }

    #[test]
    fn rules_dir_global_returns_config() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = rules_dir(&Scope::Global);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with(".gemini"));
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
            "command": "node",
            "args": ["server.js"],
            "env": {
                "API_KEY": "${MY_API_KEY}",
                "DEBUG": "true"
            },
            "timeout": 30000
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Stdio(server) = result {
            assert_eq!(server.command, "node");
            assert_eq!(server.args, vec!["server.js"]);
            assert_eq!(server.env.len(), 2);
            assert_eq!(
                server.env.get("API_KEY"),
                Some(&EnvValue::env("MY_API_KEY"))
            );
            assert_eq!(server.env.get("DEBUG"), Some(&EnvValue::plain("true")));
            assert_eq!(server.timeout_ms, Some(30000));
            assert!(server.enabled);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn parse_http_server_basic() {
        let json = json!({
            "httpUrl": "https://api.example.com/mcp"
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
    fn parse_sse_server_basic() {
        let json = json!({
            "url": "https://example.com/sse",
            "timeout": 45000
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
    fn parse_http_server_with_http_url() {
        let json = json!({
            "httpUrl": "https://example.com/http"
        });

        let result = parse_mcp_server(&json).unwrap();

        if let McpServer::Http(server) = result {
            assert_eq!(server.url, "https://example.com/http");
        } else {
            panic!("Expected Http variant");
        }
    }

    #[test]
    fn parse_mcp_server_missing_transport_fields() {
        let json = json!({
            "args": ["test"]
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_servers_missing_mcp_key_returns_empty() {
        let config = json!({
            "other_key": {}
        });

        let result = parse_mcp_servers(&config).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_mcp_servers_empty_mcp() {
        let config = json!({
            "mcpServers": {}
        });

        let result = parse_mcp_servers(&config).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn parse_mcp_servers_with_servers() {
        let config = json!({
            "mcpServers": {
                "server1": {
                    "command": "npx",
                    "args": ["-y", "server1"]
                },
                "server2": {
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
    fn parse_stdio_server_without_args() {
        let json = json!({
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
}
