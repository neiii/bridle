//! MCPB manifest.json parsing for desktop extensions.

use std::collections::HashMap;

use harness_locate::mcp::{HttpMcpServer, McpServer, StdioMcpServer};
use harness_locate::types::EnvValue;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub tools: Vec<ToolEntry>,
    #[serde(default)]
    pub user_config: Vec<UserConfigEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(rename = "type")]
    pub server_type: String,
    #[serde(flatten)]
    pub mcp_config: McpConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpConfig {
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolEntry {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserConfigEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub secret: bool,
}

impl ManifestConfig {
    pub fn required_env_vars(&self) -> Vec<&str> {
        self.user_config
            .iter()
            .filter(|e| e.required)
            .map(|e| e.id.as_str())
            .collect()
    }

    pub fn to_mcp_server(&self, _name: &str) -> Option<McpServer> {
        match self.server.server_type.as_str() {
            "stdio" => {
                let command = self.server.mcp_config.command.clone()?;
                let env: HashMap<String, EnvValue> = self
                    .server
                    .mcp_config
                    .env
                    .iter()
                    .map(|(k, v)| (k.clone(), EnvValue::plain(v)))
                    .collect();

                Some(McpServer::Stdio(StdioMcpServer {
                    command,
                    args: self.server.mcp_config.args.clone(),
                    env,
                    timeout_ms: None,
                    enabled: true,
                    cwd: None,
                }))
            }
            "streamable-http" | "http" => {
                let url = self.server.mcp_config.url.clone()?;
                Some(McpServer::Http(HttpMcpServer {
                    url,
                    headers: HashMap::new(),
                    timeout_ms: None,
                    enabled: true,
                    oauth: None,
                }))
            }
            _ => None,
        }
    }
}

pub fn parse_manifest(content: &str) -> Result<ManifestConfig, serde_json::Error> {
    serde_json::from_str(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stdio_manifest() {
        let json = r#"{
            "server": {
                "type": "stdio",
                "command": "node",
                "args": ["server.js"],
                "env": {"API_KEY": "test"}
            },
            "tools": [{"name": "search"}],
            "user_config": [
                {"id": "API_KEY", "name": "API Key", "required": true, "secret": true}
            ]
        }"#;

        let manifest = parse_manifest(json).unwrap();
        assert_eq!(manifest.server.server_type, "stdio");
        assert_eq!(manifest.server.mcp_config.command, Some("node".to_string()));
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.user_config.len(), 1);
    }

    #[test]
    fn parse_http_manifest() {
        let json = r#"{
            "server": {
                "type": "streamable-http",
                "url": "https://api.example.com/mcp"
            },
            "tools": []
        }"#;

        let manifest = parse_manifest(json).unwrap();
        assert_eq!(manifest.server.server_type, "streamable-http");
        assert_eq!(
            manifest.server.mcp_config.url,
            Some("https://api.example.com/mcp".to_string())
        );
    }

    #[test]
    fn extract_required_env_vars() {
        let json = r#"{
            "server": {"type": "stdio", "command": "test"},
            "user_config": [
                {"id": "REQUIRED_VAR", "name": "Required", "required": true},
                {"id": "OPTIONAL_VAR", "name": "Optional", "required": false}
            ]
        }"#;

        let manifest = parse_manifest(json).unwrap();
        let required = manifest.required_env_vars();
        assert_eq!(required, vec!["REQUIRED_VAR"]);
    }

    #[test]
    fn convert_to_stdio_server() {
        let json = r#"{
            "server": {
                "type": "stdio",
                "command": "python",
                "args": ["-m", "mcp_server"],
                "env": {"DEBUG": "1"}
            }
        }"#;

        let manifest = parse_manifest(json).unwrap();
        let server = manifest.to_mcp_server("test").unwrap();

        match server {
            McpServer::Stdio(s) => {
                assert_eq!(s.command, "python");
                assert_eq!(s.args, vec!["-m", "mcp_server"]);
                assert!(s.env.contains_key("DEBUG"));
            }
            _ => panic!("Expected Stdio server"),
        }
    }

    #[test]
    fn convert_to_http_server() {
        let json = r#"{
            "server": {
                "type": "streamable-http",
                "url": "https://mcp.example.com"
            }
        }"#;

        let manifest = parse_manifest(json).unwrap();
        let server = manifest.to_mcp_server("test").unwrap();

        match server {
            McpServer::Http(h) => {
                assert_eq!(h.url, "https://mcp.example.com");
            }
            _ => panic!("Expected Http server"),
        }
    }
}
