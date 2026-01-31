//! npm package.json MCP server detection.

use std::collections::HashMap;

use harness_locate::mcp::{McpServer, StdioMcpServer};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PackageJson {
    name: Option<String>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    dev_dependencies: HashMap<String, String>,
    #[serde(default)]
    bin: Option<serde_json::Value>,
}

fn is_mcp_dependency(name: &str) -> bool {
    name.starts_with("@modelcontextprotocol/") || name == "mcp" || name.starts_with("mcp-")
}

fn has_mcp_dependency(deps: &HashMap<String, String>) -> bool {
    deps.keys().any(|k| is_mcp_dependency(k))
}

fn is_mcp_package_name(name: &str) -> bool {
    name.starts_with("mcp-") || name.starts_with("@modelcontextprotocol/")
}

pub fn detect_npm_mcp(content: &str) -> Option<(String, McpServer)> {
    let pkg: PackageJson = serde_json::from_str(content).ok()?;
    let name = pkg.name.as_ref()?;

    let is_mcp = is_mcp_package_name(name)
        || has_mcp_dependency(&pkg.dependencies)
        || has_mcp_dependency(&pkg.dev_dependencies)
        || (pkg.bin.is_some() && has_mcp_dependency(&pkg.dependencies));

    if !is_mcp {
        return None;
    }

    let server = McpServer::Stdio(StdioMcpServer {
        command: "npx".to_string(),
        args: vec!["-y".to_string(), name.clone()],
        env: HashMap::new(),
        cwd: None,
        enabled: true,
        timeout_ms: None,
    });

    Some((name.clone(), server))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_modelcontextprotocol_package() {
        let json = r#"{
            "name": "@modelcontextprotocol/server-github",
            "version": "1.0.0"
        }"#;

        let result = detect_npm_mcp(json);
        assert!(result.is_some());
        let (name, server) = result.unwrap();
        assert_eq!(name, "@modelcontextprotocol/server-github");
        assert!(matches!(server, McpServer::Stdio(_)));
    }

    #[test]
    fn detect_mcp_prefixed_package() {
        let json = r#"{
            "name": "mcp-server-fetch",
            "version": "0.1.0"
        }"#;

        let result = detect_npm_mcp(json);
        assert!(result.is_some());
        let (name, _) = result.unwrap();
        assert_eq!(name, "mcp-server-fetch");
    }

    #[test]
    fn detect_package_with_mcp_dependency() {
        let json = r#"{
            "name": "my-custom-server",
            "dependencies": {
                "@modelcontextprotocol/sdk": "^1.0.0"
            }
        }"#;

        let result = detect_npm_mcp(json);
        assert!(result.is_some());
    }

    #[test]
    fn non_mcp_package_returns_none() {
        let json = r#"{
            "name": "express",
            "version": "4.18.0",
            "dependencies": {
                "body-parser": "^1.0.0"
            }
        }"#;

        let result = detect_npm_mcp(json);
        assert!(result.is_none());
    }

    #[test]
    fn invalid_json_returns_none() {
        let result = detect_npm_mcp("not valid json");
        assert!(result.is_none());
    }

    #[test]
    fn missing_name_returns_none() {
        let json = r#"{ "version": "1.0.0" }"#;
        let result = detect_npm_mcp(json);
        assert!(result.is_none());
    }
}
