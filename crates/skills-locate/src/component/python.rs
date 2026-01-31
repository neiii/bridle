//! Python pyproject.toml MCP server detection.

use std::collections::HashMap;

use harness_locate::mcp::{McpServer, StdioMcpServer};

/// Detect MCP servers from pyproject.toml content.
///
/// Looks for MCP-related packages in dependencies:
/// - `mcp` (official MCP package)
/// - `mcp-*` (MCP-prefixed packages)
/// - `*-mcp` (MCP-suffixed packages)
///
/// Returns a HashMap of server name to McpServer configuration.
pub fn detect_python_mcp(content: &str) -> HashMap<String, McpServer> {
    let mut servers = HashMap::new();

    let Ok(doc) = content.parse::<toml::Table>() else {
        return servers;
    };

    // Check [project.dependencies] array
    if let Some(project) = doc.get("project").and_then(|v| v.as_table())
        && let Some(deps) = project.get("dependencies").and_then(|v| v.as_array())
    {
        for dep in deps {
            if let Some(dep_str) = dep.as_str()
                && let Some(name) = extract_mcp_package_name(dep_str)
            {
                servers.insert(name.clone(), create_python_server(&name));
            }
        }
    }

    // Check [project.optional-dependencies.*] arrays
    if let Some(project) = doc.get("project").and_then(|v| v.as_table())
        && let Some(opt_deps) = project
            .get("optional-dependencies")
            .and_then(|v| v.as_table())
    {
        for (_group, deps) in opt_deps {
            if let Some(deps_array) = deps.as_array() {
                for dep in deps_array {
                    if let Some(dep_str) = dep.as_str()
                        && let Some(name) = extract_mcp_package_name(dep_str)
                    {
                        servers.insert(name.clone(), create_python_server(&name));
                    }
                }
            }
        }
    }

    // Check [tool.poetry.dependencies] table (Poetry format)
    if let Some(tool) = doc.get("tool").and_then(|v| v.as_table())
        && let Some(poetry) = tool.get("poetry").and_then(|v| v.as_table())
        && let Some(deps) = poetry.get("dependencies").and_then(|v| v.as_table())
    {
        for (name, _) in deps {
            if is_mcp_package(name) {
                servers.insert(name.clone(), create_python_server(name));
            }
        }
    }

    servers
}

fn extract_mcp_package_name(dep_spec: &str) -> Option<String> {
    // Parse dependency specifier: "package>=1.0" or "package[extra]>=1.0" or just "package"
    let name = dep_spec
        .split(['>', '<', '=', '[', ';', ' '])
        .next()?
        .trim();

    if is_mcp_package(name) {
        Some(name.to_string())
    } else {
        None
    }
}

fn is_mcp_package(name: &str) -> bool {
    name == "mcp" || name.starts_with("mcp-") || name.ends_with("-mcp")
}

fn create_python_server(name: &str) -> McpServer {
    McpServer::Stdio(StdioMcpServer {
        command: "python".to_string(),
        args: vec!["-m".to_string(), name.replace('-', "_")],
        env: HashMap::new(),
        timeout_ms: None,
        enabled: true,
        cwd: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mcp_in_project_dependencies() {
        let content = r#"
[project]
name = "my-project"
dependencies = [
    "mcp>=1.0",
    "requests",
]
"#;
        let servers = detect_python_mcp(content);
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("mcp"));
    }

    #[test]
    fn detects_mcp_prefixed_packages() {
        let content = r#"
[project]
dependencies = ["mcp-server-sqlite>=0.1"]
"#;
        let servers = detect_python_mcp(content);
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("mcp-server-sqlite"));
    }

    #[test]
    fn detects_mcp_suffixed_packages() {
        let content = r#"
[project]
dependencies = ["awesome-mcp"]
"#;
        let servers = detect_python_mcp(content);
        assert_eq!(servers.len(), 1);
        assert!(servers.contains_key("awesome-mcp"));
    }

    #[test]
    fn ignores_non_mcp_packages() {
        let content = r#"
[project]
dependencies = ["requests", "flask", "numpy"]
"#;
        let servers = detect_python_mcp(content);
        assert!(servers.is_empty());
    }

    #[test]
    fn detects_in_optional_dependencies() {
        let content = r#"
[project.optional-dependencies]
mcp = ["mcp>=1.0", "mcp-server-git"]
"#;
        let servers = detect_python_mcp(content);
        assert_eq!(servers.len(), 2);
    }

    #[test]
    fn detects_in_poetry_dependencies() {
        let content = r#"
[tool.poetry.dependencies]
python = "^3.11"
mcp = "^1.0"
mcp-server-fetch = { version = "^0.1", optional = true }
"#;
        let servers = detect_python_mcp(content);
        assert_eq!(servers.len(), 2);
        assert!(servers.contains_key("mcp"));
        assert!(servers.contains_key("mcp-server-fetch"));
    }

    #[test]
    fn creates_correct_server_config() {
        let content = r#"
[project]
dependencies = ["mcp-server-sqlite"]
"#;
        let servers = detect_python_mcp(content);
        let server = servers.get("mcp-server-sqlite").unwrap();

        if let McpServer::Stdio(s) = server {
            assert_eq!(s.command, "python");
            assert_eq!(s.args, vec!["-m", "mcp_server_sqlite"]);
        } else {
            panic!("Expected Stdio server");
        }
    }
}
