//! Claude Code harness implementation.
//!
//! Claude Code stores its configuration in:
//! - **Global**: `$CLAUDE_CONFIG_DIR` or `~/.claude/`
//! - **Project**: `.claude/` in project root

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::mcp::McpServer;
use crate::platform;
use crate::types::Scope;

use super::mcp_parse::{self, ParseConfig};

/// Environment variable for Claude Code config directory override.
const CLAUDE_CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";

/// Returns the global Claude Code configuration directory.
///
/// Checks `CLAUDE_CONFIG_DIR` environment variable first, then falls back
/// to `~/.claude/`.
///
/// # Errors
///
/// Returns an error if the home directory cannot be determined and
/// no environment variable is set.
pub fn global_config_dir() -> Result<PathBuf> {
    // Check environment variable first
    if let Ok(dir) = std::env::var(CLAUDE_CONFIG_DIR_ENV) {
        let path = PathBuf::from(dir);
        if path.is_absolute() {
            return Ok(path);
        }
    }

    // Fall back to ~/.claude/
    Ok(platform::home_dir()?.join(".claude"))
}

/// Returns the project-local Claude Code configuration directory.
///
/// # Arguments
///
/// * `project_root` - Path to the project root directory
#[must_use]
pub fn project_config_dir(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".claude")
}

/// Returns the commands directory for the given scope.
///
/// - **Global**: `~/.claude/commands/`
/// - **Project**: `.claude/commands/`
pub fn commands_dir(scope: &Scope) -> Result<PathBuf> {
    match scope {
        Scope::Global => Ok(global_config_dir()?.join("commands")),
        Scope::Project(root) => Ok(project_config_dir(root).join("commands")),
        Scope::Custom(path) => Ok(path.join("commands")),
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
/// Claude Code stores MCP configuration in the base config directory
/// (settings files like `.mcp.json`).
pub fn mcp_dir(scope: &Scope) -> Result<PathBuf> {
    config_dir(scope)
}

/// Returns the skills directory for the given scope.
///
/// Claude Code stores skills in nested directories with `SKILL.md` files:
/// - **Global**: `~/.claude/skills/`
/// - **Project**: `.claude/skills/`
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
/// Claude Code stores rules files (`CLAUDE.md`, `CLAUDE.local.md`) at:
/// - **Global**: `~/.claude/` (supports global `CLAUDE.md`)
/// - **Project**: Project root directory (not `.claude/`)
#[must_use]
pub fn rules_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok(),
        Scope::Project(root) => Some(root.clone()),
        Scope::Custom(path) => Some(path.clone()),
    }
}

/// Returns the agents directory for the given scope.
///
/// Claude Code stores agents as markdown files with YAML frontmatter:
/// - **Global**: `~/.claude/agents/`
/// - **Project**: `.claude/agents/`
#[must_use]
pub fn agents_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok().map(|p| p.join("agents")),
        Scope::Project(root) => Some(project_config_dir(root).join("agents")),
        Scope::Custom(path) => Some(path.join("agents")),
    }
}

/// Returns the plugins directory for the given scope.
///
/// Claude Code stores plugins as directories with a `.claude-plugin` marker:
/// - **Global**: `~/.claude/plugins/`
/// - **Project**: `.claude/plugins/`
///
/// Each plugin is a subdirectory containing a `.claude-plugin/` marker directory
/// with `plugin.json` inside.
#[must_use]
pub fn plugins_dir(scope: &Scope) -> Option<PathBuf> {
    match scope {
        Scope::Global => global_config_dir().ok().map(|p| p.join("plugins")),
        Scope::Project(root) => Some(project_config_dir(root).join("plugins")),
        Scope::Custom(path) => Some(path.join("plugins")),
    }
}

/// Checks if Claude Code is installed on this system.
///
/// Currently checks if the global config directory exists.
pub fn is_installed() -> bool {
    global_config_dir().map(|p| p.exists()).unwrap_or(false)
}

/// Parses a single MCP server from Claude Code's native JSON format.
///
/// # Arguments
/// * `value` - The JSON value representing the server config
///
/// # Errors
/// Returns an error if the JSON is malformed or missing required fields.
#[allow(dead_code)] // Internal utility for future MCP config reading
pub(crate) fn parse_mcp_server(value: &serde_json::Value) -> Result<McpServer> {
    let config = ParseConfig::CLAUDE_CODE;
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
            "stdio" => mcp_parse::parse_stdio_server(obj, &config),
            _ => Err(Error::UnsupportedMcpConfig {
                harness: config.harness_name.to_string(),
                reason: format!("Unknown server type: {}", server_type),
            }),
        }
    } else {
        mcp_parse::parse_stdio_server(obj, &config)
    }
}

/// Parses all MCP servers from a Claude Code config JSON.
///
/// # Arguments
/// * `config` - The full config JSON (expects mcpServers key)
///
/// # Errors
/// Returns an error if the JSON is malformed.
#[allow(dead_code)] // Internal utility for future MCP config reading
pub(crate) fn parse_mcp_servers(config: &serde_json::Value) -> Result<Vec<(String, McpServer)>> {
    mcp_parse::parse_servers_from_key(
        config,
        "mcpServers",
        &ParseConfig::CLAUDE_CODE,
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
        assert!(path.ends_with(".claude"));
    }

    #[test]
    fn project_config_dir_is_relative_to_root() {
        let root = PathBuf::from("/some/project");
        let config = project_config_dir(&root);
        assert_eq!(config, PathBuf::from("/some/project/.claude"));
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
        assert_eq!(path, PathBuf::from("/some/project/.claude/commands"));
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
        assert_eq!(path, PathBuf::from("/some/project/.claude/skills"));
    }

    #[test]
    fn rules_dir_global_returns_config_dir() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = rules_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with(".claude"));
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
    fn parse_stdio_server_without_args() {
        let json = json!({
            "command": "my-server"
        });

        let result = parse_mcp_server(&json);
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

        let result = parse_mcp_server(&json);
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

        let result = parse_mcp_server(&json);
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
    fn parse_mcp_server_missing_command_fails() {
        let json = json!({
            "args": ["server.js"]
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_missing_url_for_sse_fails() {
        let json = json!({
            "type": "sse"
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
    fn parse_mcp_server_not_object_fails() {
        let json = json!("not an object");

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

    #[test]
    fn parse_mcp_servers_mcp_servers_not_object_fails() {
        let config = json!({
            "mcpServers": "not an object"
        });

        let result = parse_mcp_servers(&config);
        assert!(result.is_err());
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
    fn agents_dir_project() {
        let root = PathBuf::from("/some/project");
        let result = agents_dir(&Scope::Project(root));
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.claude/agents"));
    }

    #[test]
    fn plugins_dir_global() {
        if platform::home_dir().is_err() {
            return;
        }

        let result = plugins_dir(&Scope::Global);
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.ends_with("plugins"));
    }

    #[test]
    fn plugins_dir_project() {
        let root = PathBuf::from("/some/project");
        let result = plugins_dir(&Scope::Project(root));
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, PathBuf::from("/some/project/.claude/plugins"));
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

        let result = parse_mcp_server(&json);
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
    fn roundtrip_stdio_server() {
        let original = json!({
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem"],
            "env": {
                "API_KEY": "${MY_KEY}",
                "DEBUG": "true"
            }
        });

        let parsed = parse_mcp_server(&original).unwrap();

        if let McpServer::Stdio(ref server) = parsed {
            assert_eq!(server.command, "npx");
            assert_eq!(server.args.len(), 2);
            assert_eq!(server.env.len(), 2);
        } else {
            panic!("Expected Stdio variant");
        }
    }

    #[test]
    fn roundtrip_sse_server() {
        let original = json!({
            "type": "sse",
            "url": "https://example.com/sse",
            "headers": {
                "Authorization": "${TOKEN}"
            }
        });

        let parsed = parse_mcp_server(&original).unwrap();

        if let McpServer::Sse(ref server) = parsed {
            assert_eq!(server.url, "https://example.com/sse");
            assert_eq!(server.headers.len(), 1);
        } else {
            panic!("Expected Sse variant");
        }
    }

    #[test]
    fn parse_stdio_server_with_explicit_type() {
        let json = json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "server"]
        });

        let result = parse_mcp_server(&json);
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

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_args_not_array_fails() {
        let json = json!({
            "command": "test",
            "args": "not-an-array"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_env_not_object_fails() {
        let json = json!({
            "command": "test",
            "env": "not-an-object"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_mcp_server_headers_not_object_fails() {
        let json = json!({
            "type": "sse",
            "url": "https://example.com",
            "headers": "not-an-object"
        });

        let result = parse_mcp_server(&json);
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_http_server() {
        let original = json!({
            "type": "http",
            "url": "https://api.example.com/mcp",
            "headers": {
                "X-API-Key": "${KEY}"
            }
        });

        let parsed = parse_mcp_server(&original).unwrap();

        if let McpServer::Http(ref server) = parsed {
            assert_eq!(server.url, "https://api.example.com/mcp");
            assert_eq!(server.headers.len(), 1);
        } else {
            panic!("Expected Http variant");
        }
    }
}
