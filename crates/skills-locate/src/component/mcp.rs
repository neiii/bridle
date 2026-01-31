//! MCP server descriptor types and parsing.
//!
//! Re-exports types from `harness-locate` for unified MCP representation.

use std::collections::HashMap;

use serde::Deserialize;

pub use harness_locate::{EnvValue, HttpMcpServer, McpServer, SseMcpServer, StdioMcpServer};

use crate::{Error, Result};

#[derive(Debug, Deserialize)]
struct McpServerEntry {
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    url: Option<String>,
    #[serde(rename = "type")]
    transport_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpJsonWrapped {
    mcp_servers: HashMap<String, McpServerEntry>,
}

fn convert_env(env: HashMap<String, String>) -> HashMap<String, EnvValue> {
    env.into_iter()
        .map(|(k, v)| (k, EnvValue::plain(v)))
        .collect()
}

fn entry_to_mcp_server(name: String, entry: McpServerEntry) -> Option<(String, McpServer)> {
    let transport = entry.transport_type.as_deref();

    match transport {
        Some("sse") => {
            let url = entry.url.or_else(|| entry.command.clone())?;
            Some((
                name,
                McpServer::Sse(SseMcpServer {
                    url,
                    headers: HashMap::new(),
                    timeout_ms: None,
                    enabled: true,
                }),
            ))
        }
        Some("http" | "streamable-http") => {
            let url = entry.url.or_else(|| entry.command.clone())?;
            Some((
                name,
                McpServer::Http(HttpMcpServer {
                    url,
                    headers: HashMap::new(),
                    timeout_ms: None,
                    enabled: true,
                    oauth: None,
                }),
            ))
        }
        _ => {
            let command = entry.command?;
            Some((
                name,
                McpServer::Stdio(StdioMcpServer {
                    command,
                    args: entry.args,
                    env: convert_env(entry.env),
                    timeout_ms: None,
                    enabled: true,
                    cwd: None,
                }),
            ))
        }
    }
}

fn convert_entries(map: HashMap<String, McpServerEntry>) -> HashMap<String, McpServer> {
    map.into_iter()
        .filter_map(|(name, entry)| entry_to_mcp_server(name, entry))
        .collect()
}

/// Parse a .mcp.json file content into a map of MCP servers.
///
/// Supports both formats:
/// - Wrapped: `{ "mcpServers": { "name": { ... } } }` (Claude's format)
/// - Flat: `{ "name": { ... } }` (plugin format)
///
/// Detects transport type from the `type` field:
/// - `"sse"` → SSE transport
/// - `"http"` or `"streamable-http"` → HTTP transport
/// - anything else or missing → Stdio transport
pub fn parse_mcp_json(content: &str) -> Result<HashMap<String, McpServer>> {
    if let Ok(wrapped) = serde_json::from_str::<McpJsonWrapped>(content) {
        return Ok(convert_entries(wrapped.mcp_servers));
    }

    let map: HashMap<String, McpServerEntry> =
        serde_json::from_str(content).map_err(Error::JsonParse)?;

    Ok(convert_entries(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_stdio_server() {
        let content = r#"{
            "my-server": {
                "command": "node",
                "args": ["server.js"],
                "env": {"PORT": "3000"}
            }
        }"#;
        let servers = parse_mcp_json(content).unwrap();
        assert_eq!(servers.len(), 1);
        let server = servers.get("my-server").unwrap();
        match server {
            McpServer::Stdio(s) => {
                assert_eq!(s.command, "node");
                assert_eq!(s.args, vec!["server.js"]);
                assert!(s.env.contains_key("PORT"));
            }
            _ => panic!("Expected Stdio server"),
        }
    }

    #[test]
    fn parse_multiple_servers() {
        let content = r#"{
            "server-a": {"command": "cmd-a"},
            "server-b": {"command": "cmd-b", "args": ["--flag"]}
        }"#;
        let servers = parse_mcp_json(content).unwrap();
        assert_eq!(servers.len(), 2);
    }

    #[test]
    fn parse_empty_mcp_json() {
        let content = "{}";
        let servers = parse_mcp_json(content).unwrap();
        assert!(servers.is_empty());
    }

    #[test]
    fn parse_minimal_server() {
        let content = r#"{"minimal": {"command": "echo"}}"#;
        let servers = parse_mcp_json(content).unwrap();
        assert_eq!(servers.len(), 1);
        match servers.get("minimal").unwrap() {
            McpServer::Stdio(s) => {
                assert_eq!(s.command, "echo");
                assert!(s.args.is_empty());
            }
            _ => panic!("Expected Stdio server"),
        }
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let content = "not json";
        assert!(parse_mcp_json(content).is_err());
    }

    #[test]
    fn parse_wrapped_format() {
        let content = r#"{
            "mcpServers": {
                "my-server": {
                    "command": "node",
                    "args": ["server.js"],
                    "env": {"PORT": "3000"}
                }
            }
        }"#;
        let servers = parse_mcp_json(content).unwrap();
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("my-server"));
    }

    #[test]
    fn parse_sse_server() {
        let content = r#"{
            "sse-server": {
                "type": "sse",
                "url": "http://localhost:3000/sse"
            }
        }"#;
        let servers = parse_mcp_json(content).unwrap();
        match servers.get("sse-server").unwrap() {
            McpServer::Sse(s) => {
                assert_eq!(s.url, "http://localhost:3000/sse");
            }
            _ => panic!("Expected SSE server"),
        }
    }

    #[test]
    fn parse_http_server() {
        let content = r#"{
            "http-server": {
                "type": "http",
                "url": "http://localhost:3000/mcp"
            }
        }"#;
        let servers = parse_mcp_json(content).unwrap();
        match servers.get("http-server").unwrap() {
            McpServer::Http(s) => {
                assert_eq!(s.url, "http://localhost:3000/mcp");
            }
            _ => panic!("Expected HTTP server"),
        }
    }
}
