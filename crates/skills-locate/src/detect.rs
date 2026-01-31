//! Unified MCP server detection from multiple sources.

use std::collections::HashMap;

use harness_locate::mcp::McpServer;

use crate::component::{detect_npm_mcp, detect_python_mcp, parse_manifest, parse_mcp_json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedMcp {
    pub name: String,
    pub server: McpServer,
    pub source: DetectionSource,
    pub required_env_vars: Vec<String>,
    pub confidence: DetectionConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectionSource {
    Manifest,
    McpJson,
    PackageJson,
    PyProject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DetectionConfidence {
    Low,
    Medium,
    High,
}

pub fn detect_mcp_from_files(files: &HashMap<String, String>) -> Vec<DetectedMcp> {
    let mut detected = Vec::new();

    // Priority 1: manifest.json (High confidence - explicit MCPB config)
    if let Some(content) = files.get("manifest.json")
        && let Ok(manifest) = parse_manifest(content)
    {
        let name = "mcpb-server".to_string();
        if let Some(server) = manifest.to_mcp_server(&name) {
            let env_vars = server
                .env_var_names()
                .into_iter()
                .map(String::from)
                .collect();
            detected.push(DetectedMcp {
                name,
                server,
                source: DetectionSource::Manifest,
                required_env_vars: env_vars,
                confidence: DetectionConfidence::High,
            });
        }
    }

    // Priority 2: .mcp.json or mcp.json (High confidence - explicit MCP config)
    let mcp_json_files = [".mcp.json", "mcp.json"];
    for filename in mcp_json_files {
        if let Some(content) = files.get(filename)
            && let Ok(servers) = parse_mcp_json(content)
        {
            for (name, server) in servers {
                let env_vars = server
                    .env_var_names()
                    .into_iter()
                    .map(String::from)
                    .collect();
                detected.push(DetectedMcp {
                    name,
                    server,
                    source: DetectionSource::McpJson,
                    required_env_vars: env_vars,
                    confidence: DetectionConfidence::High,
                });
            }
        }
    }

    // Priority 3: package.json (Medium confidence - inferred from dependencies)
    if let Some(content) = files.get("package.json")
        && let Some((name, server)) = detect_npm_mcp(content)
    {
        let env_vars = server
            .env_var_names()
            .into_iter()
            .map(String::from)
            .collect();
        detected.push(DetectedMcp {
            name,
            server,
            source: DetectionSource::PackageJson,
            required_env_vars: env_vars,
            confidence: DetectionConfidence::Medium,
        });
    }

    // Priority 4: pyproject.toml (Medium confidence - inferred from dependencies)
    if let Some(content) = files.get("pyproject.toml") {
        for (name, server) in detect_python_mcp(content) {
            let env_vars = server
                .env_var_names()
                .into_iter()
                .map(String::from)
                .collect();
            detected.push(DetectedMcp {
                name,
                server,
                source: DetectionSource::PyProject,
                required_env_vars: env_vars,
                confidence: DetectionConfidence::Medium,
            });
        }
    }

    detected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_from_manifest_high_confidence() {
        let mut files = HashMap::new();
        files.insert(
            "manifest.json".to_string(),
            r#"{"server": {"type": "stdio", "command": "node", "args": ["server.js"]}}"#
                .to_string(),
        );

        let detected = detect_mcp_from_files(&files);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].name, "mcpb-server");
        assert_eq!(detected[0].source, DetectionSource::Manifest);
        assert_eq!(detected[0].confidence, DetectionConfidence::High);
    }

    #[test]
    fn detect_from_mcp_json_high_confidence() {
        let mut files = HashMap::new();
        files.insert(
            ".mcp.json".to_string(),
            r#"{"my-server": {"command": "npx", "args": ["-y", "mcp-server"]}}"#.to_string(),
        );

        let detected = detect_mcp_from_files(&files);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].name, "my-server");
        assert_eq!(detected[0].source, DetectionSource::McpJson);
        assert_eq!(detected[0].confidence, DetectionConfidence::High);
    }

    #[test]
    fn detect_from_package_json_medium_confidence() {
        let mut files = HashMap::new();
        files.insert(
            "package.json".to_string(),
            r#"{"name": "test-project", "dependencies": {"@modelcontextprotocol/server-test": "^1.0.0"}}"#.to_string(),
        );

        let detected = detect_mcp_from_files(&files);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].source, DetectionSource::PackageJson);
        assert_eq!(detected[0].confidence, DetectionConfidence::Medium);
    }

    #[test]
    fn detect_from_pyproject_medium_confidence() {
        let mut files = HashMap::new();
        files.insert(
            "pyproject.toml".to_string(),
            r#"[project]
dependencies = ["mcp>=1.0.0"]"#
                .to_string(),
        );

        let detected = detect_mcp_from_files(&files);
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].source, DetectionSource::PyProject);
        assert_eq!(detected[0].confidence, DetectionConfidence::Medium);
    }

    #[test]
    fn detect_priority_ordering() {
        let mut files = HashMap::new();
        files.insert(
            "manifest.json".to_string(),
            r#"{"server": {"type": "stdio", "command": "node", "args": ["a.js"]}}"#.to_string(),
        );
        files.insert(
            ".mcp.json".to_string(),
            r#"{"mcp-server": {"command": "node", "args": ["b.js"]}}"#.to_string(),
        );
        files.insert(
            "package.json".to_string(),
            r#"{"name": "test-pkg", "dependencies": {"@modelcontextprotocol/server-npm": "^1.0.0"}}"#.to_string(),
        );

        let detected = detect_mcp_from_files(&files);
        assert_eq!(detected.len(), 3);
        assert_eq!(detected[0].source, DetectionSource::Manifest);
        assert_eq!(detected[1].source, DetectionSource::McpJson);
        assert_eq!(detected[2].source, DetectionSource::PackageJson);
    }

    #[test]
    fn detect_empty_files() {
        let files = HashMap::new();
        let detected = detect_mcp_from_files(&files);
        assert!(detected.is_empty());
    }
}
