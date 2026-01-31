//! MCP Registry API client for fetching server metadata.

use harness_locate::mcp::{HttpMcpServer, McpServer, SseMcpServer, StdioMcpServer};
use harness_locate::types::EnvValue;
use serde::Deserialize;
use std::collections::HashMap;

use crate::error::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub packages: Vec<PackageEntry>,
    #[serde(default)]
    pub remotes: Vec<RemoteEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageEntry {
    pub registry: String,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub environment_variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteEntry {
    pub transport_type: String,
    pub url: String,
}

impl ServerEntry {
    pub fn to_mcp_servers(&self) -> HashMap<String, McpServer> {
        let mut servers = HashMap::new();

        for (i, pkg) in self.packages.iter().enumerate() {
            let name = if self.packages.len() == 1 {
                self.id.clone()
            } else {
                format!("{}-{}", self.id, i)
            };

            if let Some(server) = pkg.to_mcp_server() {
                servers.insert(name, server);
            }
        }

        for (i, remote) in self.remotes.iter().enumerate() {
            let name = if self.remotes.len() == 1 && self.packages.is_empty() {
                self.id.clone()
            } else {
                format!("{}-remote-{}", self.id, i)
            };

            if let Some(server) = remote.to_mcp_server() {
                servers.insert(name, server);
            }
        }

        servers
    }
}

impl PackageEntry {
    pub fn to_mcp_server(&self) -> Option<McpServer> {
        let (command, base_args) = match (self.registry.as_str(), self.runtime.as_deref()) {
            ("npm", _) => {
                let pkg = if let Some(v) = &self.version {
                    format!("{}@{}", self.name, v)
                } else {
                    self.name.clone()
                };
                ("npx".to_string(), vec!["-y".to_string(), pkg])
            }
            ("pip" | "pypi", Some(runtime)) => {
                let pkg = if let Some(v) = &self.version {
                    format!("{}=={}", self.name, v)
                } else {
                    self.name.clone()
                };
                (
                    runtime.to_string(),
                    vec!["-m".to_string(), "pip".to_string(), "run".to_string(), pkg],
                )
            }
            ("pip" | "pypi", None) => {
                let pkg = if let Some(v) = &self.version {
                    format!("{}=={}", self.name, v)
                } else {
                    self.name.clone()
                };
                ("uvx".to_string(), vec![pkg])
            }
            _ => return None,
        };

        let mut args = base_args;
        args.extend(self.arguments.iter().cloned());

        let env: HashMap<String, EnvValue> = self
            .environment_variables
            .iter()
            .map(|(k, v)| (k.clone(), EnvValue::plain(v)))
            .collect();

        Some(McpServer::Stdio(StdioMcpServer {
            command,
            args,
            env,
            timeout_ms: None,
            enabled: true,
            cwd: None,
        }))
    }
}

impl RemoteEntry {
    pub fn to_mcp_server(&self) -> Option<McpServer> {
        match self.transport_type.as_str() {
            "sse" => Some(McpServer::Sse(SseMcpServer {
                url: self.url.clone(),
                headers: HashMap::new(),
                timeout_ms: None,
                enabled: true,
            })),
            "http" | "streamable-http" => Some(McpServer::Http(HttpMcpServer {
                url: self.url.clone(),
                headers: HashMap::new(),
                timeout_ms: None,
                enabled: true,
                oauth: None,
            })),
            _ => None,
        }
    }
}

pub struct RegistryClient {
    base_url: String,
}

impl Default for RegistryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryClient {
    pub fn new() -> Self {
        Self {
            base_url: "https://registry.modelcontextprotocol.io".to_string(),
        }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub fn fetch_server(&self, id: &str) -> Result<ServerEntry, Error> {
        let url = format!("{}/servers/{}", self.base_url, id);
        let mut response = ureq::get(&url)
            .call()
            .map_err(|e| Error::Http(e.to_string()))?;
        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| Error::Http(format!("Failed to read registry response: {e}")))?;
        let entry: ServerEntry = serde_json::from_slice(&bytes)
            .map_err(|e| Error::Http(format!("Failed to parse registry response: {e}")))?;
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_entry_npm_to_mcp_server() {
        let pkg = PackageEntry {
            registry: "npm".to_string(),
            name: "@modelcontextprotocol/server-filesystem".to_string(),
            version: Some("1.0.0".to_string()),
            runtime: None,
            arguments: vec!["--root".to_string(), "/tmp".to_string()],
            environment_variables: HashMap::new(),
        };

        let server = pkg.to_mcp_server().expect("should create server");
        match server {
            McpServer::Stdio(s) => {
                assert_eq!(s.command, "npx");
                assert_eq!(
                    s.args,
                    vec![
                        "-y",
                        "@modelcontextprotocol/server-filesystem@1.0.0",
                        "--root",
                        "/tmp"
                    ]
                );
            }
            _ => panic!("expected Stdio"),
        }
    }

    #[test]
    fn package_entry_pip_to_mcp_server() {
        let pkg = PackageEntry {
            registry: "pip".to_string(),
            name: "mcp-server-fetch".to_string(),
            version: None,
            runtime: None,
            arguments: vec![],
            environment_variables: HashMap::new(),
        };

        let server = pkg.to_mcp_server().expect("should create server");
        match server {
            McpServer::Stdio(s) => {
                assert_eq!(s.command, "uvx");
                assert_eq!(s.args, vec!["mcp-server-fetch"]);
            }
            _ => panic!("expected Stdio"),
        }
    }

    #[test]
    fn remote_entry_sse_to_mcp_server() {
        let remote = RemoteEntry {
            transport_type: "sse".to_string(),
            url: "https://example.com/sse".to_string(),
        };

        let server = remote.to_mcp_server().expect("should create server");
        match server {
            McpServer::Sse(s) => {
                assert_eq!(s.url, "https://example.com/sse");
            }
            _ => panic!("expected Sse"),
        }
    }

    #[test]
    fn remote_entry_http_to_mcp_server() {
        let remote = RemoteEntry {
            transport_type: "http".to_string(),
            url: "https://example.com/mcp".to_string(),
        };

        let server = remote.to_mcp_server().expect("should create server");
        match server {
            McpServer::Http(s) => {
                assert_eq!(s.url, "https://example.com/mcp");
            }
            _ => panic!("expected Http"),
        }
    }

    #[test]
    fn server_entry_to_mcp_servers() {
        let entry = ServerEntry {
            id: "test-server".to_string(),
            name: "Test Server".to_string(),
            description: None,
            packages: vec![PackageEntry {
                registry: "npm".to_string(),
                name: "test-pkg".to_string(),
                version: None,
                runtime: None,
                arguments: vec![],
                environment_variables: HashMap::new(),
            }],
            remotes: vec![RemoteEntry {
                transport_type: "http".to_string(),
                url: "https://example.com".to_string(),
            }],
        };

        let servers = entry.to_mcp_servers();
        assert_eq!(servers.len(), 2);
        assert!(servers.contains_key("test-server"));
        assert!(servers.contains_key("test-server-remote-0"));
    }
}
