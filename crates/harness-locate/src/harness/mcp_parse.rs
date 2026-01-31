//! Shared MCP server parsing utilities.
//!
//! This module contains common parsing logic used across multiple harness implementations
//! to reduce code duplication when parsing MCP server configurations from JSON.

use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::mcp::{HttpMcpServer, McpServer, OAuthConfig, SseMcpServer, StdioMcpServer};
use crate::types::{EnvValue, HarnessKind};

/// Configuration for MCP server parsing behavior.
#[derive(Debug, Clone)]
pub struct ParseConfig {
    /// Harness name for error messages.
    pub harness_name: &'static str,
    /// Harness kind for env value parsing.
    pub harness_kind: HarnessKind,
    /// Field name for args array (e.g., "args" for most, "args" for all).
    pub args_field: &'static str,
    /// Field name for env variables (e.g., "env", "envs", "environment").
    pub env_field: &'static str,
    /// Field name for command (e.g., "command" or "cmd").
    pub command_field: &'static str,
    /// Field name for URL (e.g., "url" or "uri").
    pub url_field: &'static str,
    /// Whether to use plain env values (no env var parsing).
    pub plain_env_values: bool,
    /// Field name for enabled flag, if inverted (e.g., "disabled" -> invert).
    pub disabled_field: Option<&'static str>,
    /// Field name for timeout in milliseconds.
    pub timeout_field: &'static str,
    /// Whether timeout is in seconds (needs conversion to ms).
    pub timeout_in_seconds: bool,
}

impl ParseConfig {
    /// Claude Code style parsing config.
    pub const CLAUDE_CODE: Self = Self {
        harness_name: "Claude Code",
        harness_kind: HarnessKind::ClaudeCode,
        args_field: "args",
        env_field: "env",
        command_field: "command",
        url_field: "url",
        plain_env_values: false,
        disabled_field: None,
        timeout_field: "timeout",
        timeout_in_seconds: false,
    };

    /// OpenCode style parsing config.
    pub const OPENCODE: Self = Self {
        harness_name: "OpenCode",
        harness_kind: HarnessKind::OpenCode,
        args_field: "command", // OpenCode uses command array
        env_field: "environment",
        command_field: "command",
        url_field: "url",
        plain_env_values: false,
        disabled_field: None,
        timeout_field: "timeout",
        timeout_in_seconds: false,
    };

    /// Goose style parsing config.
    pub const GOOSE: Self = Self {
        harness_name: "Goose",
        harness_kind: HarnessKind::Goose,
        args_field: "args",
        env_field: "envs",
        command_field: "cmd",
        url_field: "uri",
        plain_env_values: true,
        disabled_field: None,
        timeout_field: "timeout",
        timeout_in_seconds: true,
    };

    /// Crush style parsing config.
    pub const CRUSH: Self = Self {
        harness_name: "Crush",
        harness_kind: HarnessKind::Crush,
        args_field: "args",
        env_field: "env",
        command_field: "command",
        url_field: "url",
        plain_env_values: true,
        disabled_field: Some("disabled"),
        timeout_field: "timeout_ms",
        timeout_in_seconds: false,
    };

    /// Droid style parsing config.
    pub const DROID: Self = Self {
        harness_name: "Droid",
        harness_kind: HarnessKind::Droid,
        args_field: "args",
        env_field: "env",
        command_field: "command",
        url_field: "url",
        plain_env_values: false,
        disabled_field: Some("disabled"),
        timeout_field: "timeout",
        timeout_in_seconds: false,
    };

    /// AMP Code style parsing config.
    pub const AMP_CODE: Self = Self {
        harness_name: "AMP Code",
        harness_kind: HarnessKind::AmpCode,
        args_field: "args",
        env_field: "env",
        command_field: "command",
        url_field: "url",
        plain_env_values: false,
        disabled_field: None,
        timeout_field: "timeout",
        timeout_in_seconds: false,
    };

    /// Copilot CLI style parsing config.
    pub const COPILOT_CLI: Self = Self {
        harness_name: "Copilot CLI",
        harness_kind: HarnessKind::CopilotCli,
        args_field: "args",
        env_field: "env",
        command_field: "command",
        url_field: "url",
        plain_env_values: false,
        disabled_field: None,
        timeout_field: "timeout",
        timeout_in_seconds: false,
    };
}

/// Parse a string array from a JSON object field.
pub fn parse_string_array(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    harness: &str,
) -> Result<Vec<String>> {
    let Some(value) = obj.get(field) else {
        return Ok(Vec::new());
    };

    let arr = value
        .as_array()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: harness.to_string(),
            reason: format!("'{}' must be an array", field),
        })?;

    arr.iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_str()
                .ok_or_else(|| Error::UnsupportedMcpConfig {
                    harness: harness.to_string(),
                    reason: format!("{}[{}] must be a string", field, i),
                })
                .map(String::from)
        })
        .collect()
}

/// Parse environment variables or headers from a JSON object field.
pub fn parse_env_map(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    harness_name: &str,
    harness_kind: HarnessKind,
    plain_values: bool,
) -> Result<HashMap<String, EnvValue>> {
    let Some(value) = obj.get(field) else {
        return Ok(HashMap::new());
    };

    let map_obj = value
        .as_object()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: harness_name.to_string(),
            reason: format!("'{}' must be an object", field),
        })?;

    let mut result = HashMap::new();
    for (key, value) in map_obj {
        let value_str = value.as_str().ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: harness_name.to_string(),
            reason: format!("{}.{} must be a string", field, key),
        })?;

        let env_value = if plain_values {
            EnvValue::plain(value_str)
        } else {
            EnvValue::from_native(value_str, harness_kind)
        };

        result.insert(key.clone(), env_value);
    }

    Ok(result)
}

/// Parse timeout value from JSON object.
pub fn parse_timeout(
    obj: &serde_json::Map<String, serde_json::Value>,
    field: &str,
    in_seconds: bool,
    harness: &str,
) -> Result<Option<u64>> {
    let Some(value) = obj.get(field) else {
        return Ok(None);
    };

    let raw = value.as_u64().ok_or_else(|| Error::UnsupportedMcpConfig {
        harness: harness.to_string(),
        reason: format!("'{}' must be a number", field),
    })?;

    if in_seconds {
        raw.checked_mul(1000)
            .map(Some)
            .ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: harness.to_string(),
                reason: "timeout value too large".to_string(),
            })
    } else {
        Ok(Some(raw))
    }
}

/// Parse enabled/disabled flag from JSON object.
pub fn parse_enabled(
    obj: &serde_json::Map<String, serde_json::Value>,
    disabled_field: Option<&str>,
) -> bool {
    if let Some(field) = disabled_field {
        !obj.get(field).and_then(|v| v.as_bool()).unwrap_or(false)
    } else {
        obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true)
    }
}

/// Parse a stdio MCP server from JSON.
pub fn parse_stdio_server(
    obj: &serde_json::Map<String, serde_json::Value>,
    config: &ParseConfig,
) -> Result<McpServer> {
    let command = obj
        .get(config.command_field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: format!("Stdio server missing '{}' field", config.command_field),
        })?
        .to_string();

    let args = parse_string_array(obj, config.args_field, config.harness_name)?;
    let env = parse_env_map(
        obj,
        config.env_field,
        config.harness_name,
        config.harness_kind,
        config.plain_env_values,
    )?;
    let timeout_ms = parse_timeout(
        obj,
        config.timeout_field,
        config.timeout_in_seconds,
        config.harness_name,
    )?;
    let enabled = parse_enabled(obj, config.disabled_field);

    Ok(McpServer::Stdio(StdioMcpServer {
        command,
        args,
        env,
        cwd: None,
        enabled,
        timeout_ms,
    }))
}

/// Parse an SSE MCP server from JSON.
pub fn parse_sse_server(
    obj: &serde_json::Map<String, serde_json::Value>,
    config: &ParseConfig,
) -> Result<McpServer> {
    let url = obj
        .get(config.url_field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: format!("SSE server missing '{}' field", config.url_field),
        })?
        .to_string();

    let headers = parse_env_map(
        obj,
        "headers",
        config.harness_name,
        config.harness_kind,
        config.plain_env_values,
    )?;
    let timeout_ms = parse_timeout(
        obj,
        config.timeout_field,
        config.timeout_in_seconds,
        config.harness_name,
    )?;
    let enabled = parse_enabled(obj, config.disabled_field);

    Ok(McpServer::Sse(SseMcpServer {
        url,
        headers,
        enabled,
        timeout_ms,
    }))
}

/// Parse an HTTP MCP server from JSON.
pub fn parse_http_server(
    obj: &serde_json::Map<String, serde_json::Value>,
    config: &ParseConfig,
) -> Result<McpServer> {
    let url = obj
        .get(config.url_field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: format!("HTTP server missing '{}' field", config.url_field),
        })?
        .to_string();

    let headers = parse_env_map(
        obj,
        "headers",
        config.harness_name,
        config.harness_kind,
        config.plain_env_values,
    )?;
    let timeout_ms = parse_timeout(
        obj,
        config.timeout_field,
        config.timeout_in_seconds,
        config.harness_name,
    )?;
    let enabled = parse_enabled(obj, config.disabled_field);

    Ok(McpServer::Http(HttpMcpServer {
        url,
        headers,
        oauth: None,
        enabled,
        timeout_ms,
    }))
}

/// Parse OAuth configuration from JSON.
pub fn parse_oauth(
    obj: &serde_json::Map<String, serde_json::Value>,
    config: &ParseConfig,
) -> Result<Option<OAuthConfig>> {
    let Some(oauth_value) = obj.get("oauth") else {
        return Ok(None);
    };

    let oauth_obj = oauth_value
        .as_object()
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: config.harness_name.to_string(),
            reason: "'oauth' must be an object".to_string(),
        })?;

    let client_id = if let Some(v) = oauth_obj.get("client_id") {
        Some(
            v.as_str()
                .ok_or_else(|| Error::UnsupportedMcpConfig {
                    harness: config.harness_name.to_string(),
                    reason: "oauth.client_id must be a string".to_string(),
                })?
                .to_string(),
        )
    } else {
        None
    };

    let client_secret = if let Some(v) = oauth_obj.get("client_secret") {
        Some(EnvValue::from_native(
            v.as_str().ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: config.harness_name.to_string(),
                reason: "oauth.client_secret must be a string".to_string(),
            })?,
            config.harness_kind,
        ))
    } else {
        None
    };

    let scope = if let Some(v) = oauth_obj.get("scope") {
        Some(
            v.as_str()
                .ok_or_else(|| Error::UnsupportedMcpConfig {
                    harness: config.harness_name.to_string(),
                    reason: "oauth.scope must be a string".to_string(),
                })?
                .to_string(),
        )
    } else {
        None
    };

    Ok(Some(OAuthConfig {
        client_id,
        client_secret,
        scope,
    }))
}

/// Parse servers from a JSON config using a given root key.
pub fn parse_servers_from_key(
    config: &serde_json::Value,
    root_key: &str,
    parse_config: &ParseConfig,
    parse_fn: impl Fn(&serde_json::Value) -> Result<McpServer>,
) -> Result<Vec<(String, McpServer)>> {
    let servers_obj = config
        .get(root_key)
        .and_then(|v| v.as_object())
        .ok_or_else(|| Error::UnsupportedMcpConfig {
            harness: parse_config.harness_name.to_string(),
            reason: format!("Missing '{}' key", root_key),
        })?;

    let mut result = Vec::new();
    for (name, value) in servers_obj {
        let server = parse_fn(value).map_err(|e| Error::UnsupportedMcpConfig {
            harness: parse_config.harness_name.to_string(),
            reason: format!("server '{}': {}", name, e),
        })?;
        result.push((name.clone(), server));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_string_array_basic() {
        let obj = json!({
            "args": ["a", "b", "c"]
        });
        let obj = obj.as_object().unwrap();

        let result = parse_string_array(obj, "args", "test").unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn parse_string_array_empty() {
        let obj = json!({});
        let obj = obj.as_object().unwrap();

        let result = parse_string_array(obj, "args", "test").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_env_map_plain() {
        let obj = json!({
            "env": {
                "KEY": "value"
            }
        });
        let obj = obj.as_object().unwrap();

        let result = parse_env_map(obj, "env", "test", HarnessKind::Goose, true).unwrap();
        assert_eq!(result.get("KEY"), Some(&EnvValue::plain("value")));
    }

    #[test]
    fn parse_env_map_with_env_ref() {
        let obj = json!({
            "env": {
                "KEY": "${MY_VAR}"
            }
        });
        let obj = obj.as_object().unwrap();

        let result = parse_env_map(obj, "env", "test", HarnessKind::ClaudeCode, false).unwrap();
        assert_eq!(result.get("KEY"), Some(&EnvValue::env("MY_VAR")));
    }

    #[test]
    fn parse_timeout_in_seconds() {
        let obj = json!({
            "timeout": 30
        });
        let obj = obj.as_object().unwrap();

        let result = parse_timeout(obj, "timeout", true, "test").unwrap();
        assert_eq!(result, Some(30000));
    }

    #[test]
    fn parse_timeout_in_ms() {
        let obj = json!({
            "timeout_ms": 5000
        });
        let obj = obj.as_object().unwrap();

        let result = parse_timeout(obj, "timeout_ms", false, "test").unwrap();
        assert_eq!(result, Some(5000));
    }

    #[test]
    fn parse_enabled_default_true() {
        let obj = json!({});
        let obj = obj.as_object().unwrap();

        assert!(parse_enabled(obj, None));
    }

    #[test]
    fn parse_enabled_explicit_false() {
        let obj = json!({
            "enabled": false
        });
        let obj = obj.as_object().unwrap();

        assert!(!parse_enabled(obj, None));
    }

    #[test]
    fn parse_disabled_field() {
        let obj = json!({
            "disabled": true
        });
        let obj = obj.as_object().unwrap();

        assert!(!parse_enabled(obj, Some("disabled")));
    }

    #[test]
    fn parse_stdio_server_basic() {
        let obj = json!({
            "command": "node",
            "args": ["server.js"]
        });
        let obj = obj.as_object().unwrap();

        let server = parse_stdio_server(obj, &ParseConfig::CLAUDE_CODE).unwrap();
        if let McpServer::Stdio(s) = server {
            assert_eq!(s.command, "node");
            assert_eq!(s.args, vec!["server.js"]);
        } else {
            panic!("Expected stdio server");
        }
    }

    #[test]
    fn parse_stdio_server_goose_style() {
        let obj = json!({
            "cmd": "npx",
            "args": ["-y", "server"],
            "envs": {
                "KEY": "value"
            },
            "timeout": 30
        });
        let obj = obj.as_object().unwrap();

        let server = parse_stdio_server(obj, &ParseConfig::GOOSE).unwrap();
        if let McpServer::Stdio(s) = server {
            assert_eq!(s.command, "npx");
            assert_eq!(s.args, vec!["-y", "server"]);
            assert_eq!(s.env.get("KEY"), Some(&EnvValue::plain("value")));
            assert_eq!(s.timeout_ms, Some(30000));
        } else {
            panic!("Expected stdio server");
        }
    }

    #[test]
    fn parse_sse_server_basic() {
        let obj = json!({
            "url": "https://example.com/sse"
        });
        let obj = obj.as_object().unwrap();

        let server = parse_sse_server(obj, &ParseConfig::CLAUDE_CODE).unwrap();
        if let McpServer::Sse(s) = server {
            assert_eq!(s.url, "https://example.com/sse");
        } else {
            panic!("Expected SSE server");
        }
    }

    #[test]
    fn parse_http_server_basic() {
        let obj = json!({
            "url": "https://api.example.com/mcp"
        });
        let obj = obj.as_object().unwrap();

        let server = parse_http_server(obj, &ParseConfig::CLAUDE_CODE).unwrap();
        if let McpServer::Http(s) = server {
            assert_eq!(s.url, "https://api.example.com/mcp");
        } else {
            panic!("Expected HTTP server");
        }
    }

    #[test]
    fn parse_oauth_config() {
        let obj = json!({
            "oauth": {
                "client_id": "app",
                "client_secret": "{env:SECRET}",
                "scope": "read write"
            }
        });
        let obj = obj.as_object().unwrap();

        let oauth = parse_oauth(obj, &ParseConfig::OPENCODE).unwrap();
        assert!(oauth.is_some());
        let oauth = oauth.unwrap();
        assert_eq!(oauth.client_id, Some("app".to_string()));
        assert_eq!(oauth.client_secret, Some(EnvValue::env("SECRET")));
        assert_eq!(oauth.scope, Some("read write".to_string()));
    }
}
