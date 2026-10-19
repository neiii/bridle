//! Shared helpers for harnesses with a single dot-directory layout.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::mcp::McpServer;
use crate::platform;
use crate::types::Scope;

use super::mcp_parse::{self, ParseConfig};

#[derive(Debug, Clone, Copy)]
pub(crate) struct DotDirHarness {
    pub(crate) dot_dir: &'static str,
    pub(crate) agents_dir: &'static str,
    pub(crate) mcp_key: &'static str,
    pub(crate) mcp_config: &'static ParseConfig,
}

impl DotDirHarness {
    pub(crate) fn global_config_dir(self) -> Result<PathBuf> {
        Ok(platform::home_dir()?.join(self.dot_dir))
    }

    pub(crate) fn project_config_dir(self, project_root: &Path) -> PathBuf {
        project_root.join(self.dot_dir)
    }

    pub(crate) fn config_dir(self, scope: &Scope) -> Result<PathBuf> {
        match scope {
            Scope::Global => self.global_config_dir(),
            Scope::Project(root) => Ok(self.project_config_dir(root)),
            Scope::Custom(path) => Ok(path.clone()),
        }
    }

    pub(crate) fn child_dir(self, scope: &Scope, child: &str) -> Result<PathBuf> {
        Ok(self.config_dir(scope)?.join(child))
    }

    pub(crate) fn optional_child_dir(self, scope: &Scope, child: &str) -> Option<PathBuf> {
        match scope {
            Scope::Global => self.global_config_dir().ok().map(|p| p.join(child)),
            Scope::Project(root) => Some(self.project_config_dir(root).join(child)),
            Scope::Custom(path) => Some(path.join(child)),
        }
    }

    pub(crate) fn rules_dir(self, scope: &Scope) -> Option<PathBuf> {
        match scope {
            Scope::Global => self.global_config_dir().ok(),
            Scope::Project(root) => Some(root.clone()),
            Scope::Custom(path) => Some(path.clone()),
        }
    }

    pub(crate) fn agents_dir(self, scope: &Scope) -> Option<PathBuf> {
        self.optional_child_dir(scope, self.agents_dir)
    }

    pub(crate) fn is_installed(self) -> bool {
        self.global_config_dir()
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    pub(crate) fn parse_mcp_server(self, value: &serde_json::Value) -> Result<McpServer> {
        let config = self.mcp_config;
        let obj = value
            .as_object()
            .ok_or_else(|| Error::UnsupportedMcpConfig {
                harness: config.harness_name.to_string(),
                reason: "Server configuration must be an object".to_string(),
            })?;

        if let Some(server_type) = obj.get("type").and_then(|v| v.as_str()) {
            match server_type {
                "http" => mcp_parse::parse_http_server(obj, config),
                "stdio" => mcp_parse::parse_stdio_server(obj, config),
                _ => Err(Error::UnsupportedMcpConfig {
                    harness: config.harness_name.to_string(),
                    reason: format!("Unknown server type: {server_type}"),
                }),
            }
        } else if obj.contains_key("url") {
            mcp_parse::parse_sse_server(obj, config)
        } else {
            mcp_parse::parse_stdio_server(obj, config)
        }
    }

    pub(crate) fn parse_mcp_servers(
        self,
        config: &serde_json::Value,
        parse_server: fn(&serde_json::Value) -> Result<McpServer>,
    ) -> Result<Vec<(String, McpServer)>> {
        mcp_parse::parse_servers_from_key(config, self.mcp_key, self.mcp_config, parse_server)
    }
}
