//! Harness integration for bridle.

#![allow(dead_code)]
#![allow(unused_imports)]

mod adapter;
mod display;

use std::path::PathBuf;

use harness_locate::{InstallationStatus, McpServer, Scope};

use crate::error::Result;

pub use adapter::HarnessAdapter;
pub use display::DisplayInfo;

pub trait HarnessConfig {
    fn id(&self) -> &str;
    fn config_dir(&self) -> Result<PathBuf>;
    fn installation_status(&self) -> Result<InstallationStatus>;
    fn mcp_filename(&self) -> Option<String>;
    fn parse_mcp_servers(&self, content: &str, filename: &str) -> Result<Vec<(String, bool)>>;
}

fn mcp_server_enabled(server: &McpServer) -> bool {
    match server {
        McpServer::Stdio(s) => s.enabled,
        McpServer::Sse(s) => s.enabled,
        McpServer::Http(s) => s.enabled,
    }
}

impl HarnessConfig for harness_locate::Harness {
    fn id(&self) -> &'static str {
        match self.kind() {
            harness_locate::HarnessKind::ClaudeCode => "claude-code",
            harness_locate::HarnessKind::OpenCode => "opencode",
            harness_locate::HarnessKind::Goose => "goose",
            _ => "unknown",
        }
    }

    fn config_dir(&self) -> Result<PathBuf> {
        Ok(self.config(&Scope::Global)?)
    }

    fn installation_status(&self) -> Result<InstallationStatus> {
        Ok(harness_locate::Harness::installation_status(self)?)
    }

    fn mcp_filename(&self) -> Option<String> {
        self.mcp(&Scope::Global)
            .ok()
            .flatten()
            .map(|r| r.file)
            .and_then(|f| f.file_name().map(|n| n.to_os_string()))
            .and_then(|n| n.into_string().ok())
    }

    fn parse_mcp_servers(&self, content: &str, filename: &str) -> Result<Vec<(String, bool)>> {
        let is_yaml = filename.ends_with(".yaml") || filename.ends_with(".yml");
        let mut parsed: serde_json::Value = if is_yaml {
            let yaml: serde_yaml::Value = serde_yaml::from_str(content)?;
            serde_json::to_value(yaml)?
        } else {
            serde_json::from_str(content)?
        };

        // For Goose, filter extensions to only include actual MCP server types
        // (exclude builtin/platform which are Goose-internal, not MCP)
        if self.id() == "goose"
            && let Some(extensions) = parsed.get_mut("extensions")
            && let Some(ext_obj) = extensions.as_object_mut()
        {
            let mcp_types = ["stdio", "sse", "http", "streamable_http"];
            ext_obj.retain(|_, v| {
                v.get("type")
                    .and_then(|t| t.as_str())
                    .is_some_and(|t| mcp_types.contains(&t))
            });
        }

        let servers: std::collections::HashMap<String, McpServer> =
            self.parse_mcp_config(&parsed)?;
        let mut result: Vec<(String, bool)> = servers
            .iter()
            .map(|(name, server)| (name.clone(), mcp_server_enabled(server)))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }
}
