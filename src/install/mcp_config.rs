//! MCP configuration read/write helpers for all harnesses.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use harness_locate::HarnessKind;

use crate::config::jsonc::strip_jsonc_comments;

#[derive(Debug, thiserror::Error)]
pub enum McpConfigError {
    #[error("Failed to read config file: {0}")]
    Read(#[from] std::io::Error),

    #[error("Failed to parse JSON: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Failed to parse YAML: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("Failed to write config: {0}")]
    Write(String),
}

fn get_mcp_key(kind: HarnessKind) -> &'static str {
    match kind {
        HarnessKind::ClaudeCode => "mcpServers",
        HarnessKind::OpenCode => "mcp",
        HarnessKind::Goose => "extensions",
        HarnessKind::AmpCode => "amp.mcpServers",
        _ => "mcpServers",
    }
}

pub fn read_mcp_config(
    kind: HarnessKind,
    config_path: &Path,
) -> Result<HashMap<String, serde_json::Value>, McpConfigError> {
    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(config_path)?;
    if content.trim().is_empty() {
        return Ok(HashMap::new());
    }

    let parsed: serde_json::Value = match kind {
        HarnessKind::Goose => {
            let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;
            serde_json::to_value(yaml)?
        }
        HarnessKind::OpenCode => {
            let stripped = strip_jsonc_comments(&content);
            serde_json::from_str(&stripped)?
        }
        _ => serde_json::from_str(&content)?,
    };

    let key = get_mcp_key(kind);
    let mcp_section = parsed.get(key).and_then(|v| v.as_object());

    match mcp_section {
        Some(obj) => {
            let mut result = HashMap::new();
            for (name, value) in obj {
                if kind == HarnessKind::Goose {
                    if let Some(ext_type) = value.get("type").and_then(|t| t.as_str()) {
                        if !["stdio", "sse", "http", "streamable_http"].contains(&ext_type) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                result.insert(name.clone(), value.clone());
            }
            Ok(result)
        }
        None => Ok(HashMap::new()),
    }
}

pub fn write_mcp_config(
    kind: HarnessKind,
    config_path: &Path,
    servers: &HashMap<String, serde_json::Value>,
) -> Result<(), McpConfigError> {
    if kind == HarnessKind::Goose {
        return write_goose_yaml_preserving_comments(config_path, servers);
    }

    let key = get_mcp_key(kind);

    let mut existing: serde_json::Value = if config_path.exists() {
        let content = fs::read_to_string(config_path)?;
        if content.trim().is_empty() {
            serde_json::json!({})
        } else {
            match kind {
                HarnessKind::OpenCode => {
                    let stripped = strip_jsonc_comments(&content);
                    serde_json::from_str(&stripped)?
                }
                _ => serde_json::from_str(&content)?,
            }
        }
    } else {
        serde_json::json!({})
    };

    let mcp_section = existing
        .as_object_mut()
        .ok_or_else(|| McpConfigError::Write("Config root is not an object".to_string()))?
        .entry(key)
        .or_insert_with(|| serde_json::json!({}));

    let mcp_obj = mcp_section
        .as_object_mut()
        .ok_or_else(|| McpConfigError::Write(format!("{} section is not an object", key)))?;

    for (name, value) in servers {
        mcp_obj.insert(name.clone(), value.clone());
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let output = serde_json::to_string_pretty(&existing)?;
    fs::write(config_path, output)?;
    Ok(())
}

fn write_goose_yaml_preserving_comments(
    config_path: &Path,
    servers: &HashMap<String, serde_json::Value>,
) -> Result<(), McpConfigError> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = if config_path.exists() {
        fs::read_to_string(config_path)?
    } else {
        String::new()
    };

    let mut output = if content.trim().is_empty() {
        String::new()
    } else {
        content.clone()
    };

    for (name, value) in servers {
        if mcp_entry_exists_in_yaml(&output, name) {
            continue;
        }
        let yaml_entry = format_goose_mcp_entry(name, value);
        output = insert_into_extensions_section(&output, &yaml_entry);
    }

    fs::write(config_path, output)?;
    Ok(())
}

fn mcp_entry_exists_in_yaml(content: &str, name: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("{}:", name))
            || trimmed.starts_with(&format!("\"{}\":", name))
        {
            return true;
        }
    }
    false
}

fn format_goose_mcp_entry(name: &str, value: &serde_json::Value) -> String {
    let mut lines = vec![format!("  {}:", name)];

    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            match v {
                serde_json::Value::String(s) => {
                    lines.push(format!("    {}: {}", k, s));
                }
                serde_json::Value::Array(arr) => {
                    let items: Vec<String> = arr
                        .iter()
                        .filter_map(|item| item.as_str().map(|s| format!("\"{}\"", s)))
                        .collect();
                    lines.push(format!("    {}: [{}]", k, items.join(", ")));
                }
                serde_json::Value::Bool(b) => {
                    lines.push(format!("    {}: {}", k, b));
                }
                serde_json::Value::Number(n) => {
                    lines.push(format!("    {}: {}", k, n));
                }
                serde_json::Value::Object(map) => {
                    lines.push(format!("    {}:", k));
                    for (inner_k, inner_v) in map {
                        if let Some(s) = inner_v.as_str() {
                            lines.push(format!("      {}: {}", inner_k, s));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    lines.join("\n")
}

fn insert_into_extensions_section(content: &str, entry: &str) -> String {
    if content.trim().is_empty() {
        return format!("extensions:\n{}\n", entry);
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut found_extensions = false;
    let mut inserted = false;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        result.push(line.to_string());

        if line.trim() == "extensions:" || line.trim().starts_with("extensions:") {
            found_extensions = true;
            i += 1;

            while i < lines.len() {
                let next_line = lines[i];
                let is_indented = next_line.starts_with("  ") || next_line.starts_with("\t");
                let is_empty = next_line.trim().is_empty();
                let is_comment = next_line.trim().starts_with('#');

                if is_indented || is_empty || is_comment {
                    result.push(next_line.to_string());
                    i += 1;
                } else {
                    break;
                }
            }

            result.push(entry.to_string());
            inserted = true;
            continue;
        }
        i += 1;
    }

    if !found_extensions {
        if !result.is_empty() && !result.last().map(|s| s.is_empty()).unwrap_or(true) {
            result.push(String::new());
        }
        result.push("extensions:".to_string());
        result.push(entry.to_string());
        inserted = true;
    }

    let mut output = result.join("\n");
    if !output.ends_with('\n') && inserted {
        output.push('\n');
    }
    output
}

pub fn mcp_exists(
    kind: HarnessKind,
    config_path: &Path,
    name: &str,
) -> Result<bool, McpConfigError> {
    let servers = read_mcp_config(kind, config_path)?;
    Ok(servers.contains_key(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_nonexistent_file_returns_empty() {
        let result = read_mcp_config(HarnessKind::ClaudeCode, Path::new("/nonexistent/path.json"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn read_empty_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(&path, "").unwrap();

        let result = read_mcp_config(HarnessKind::ClaudeCode, &path);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn read_claude_mcp_config() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers": {"test-server": {"command": "test"}}}"#,
        )
        .unwrap();

        let result = read_mcp_config(HarnessKind::ClaudeCode, &path).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("test-server"));
    }

    #[test]
    fn read_opencode_jsonc_config() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("opencode.jsonc");
        fs::write(
            &path,
            r#"{
                // This is a comment
                "mcp": {
                    "my-mcp": {"command": "npx", "args": ["-y", "server"]}
                }
            }"#,
        )
        .unwrap();

        let result = read_mcp_config(HarnessKind::OpenCode, &path).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("my-mcp"));
    }

    #[test]
    fn read_goose_yaml_filters_mcp_types() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.yaml");
        fs::write(
            &path,
            r#"
extensions:
  developer:
    enabled: true
    type: builtin
  my-mcp:
    type: stdio
    cmd: npx
    args: ["-y", "server"]
"#,
        )
        .unwrap();

        let result = read_mcp_config(HarnessKind::Goose, &path).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("my-mcp"));
        assert!(!result.contains_key("developer"));
    }

    #[test]
    fn read_amp_config() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        fs::write(
            &path,
            r#"{"amp.mcpServers": {"amp-mcp": {"command": "test"}}}"#,
        )
        .unwrap();

        let result = read_mcp_config(HarnessKind::AmpCode, &path).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("amp-mcp"));
    }

    #[test]
    fn write_creates_file_if_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("new-config.json");

        let mut servers = HashMap::new();
        servers.insert(
            "new-server".to_string(),
            serde_json::json!({"command": "test"}),
        );

        write_mcp_config(HarnessKind::ClaudeCode, &path, &servers).unwrap();

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("new-server"));
    }

    #[test]
    fn write_preserves_existing_mcps() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(&path, r#"{"mcpServers": {"existing": {"command": "old"}}}"#).unwrap();

        let mut servers = HashMap::new();
        servers.insert(
            "new-server".to_string(),
            serde_json::json!({"command": "new"}),
        );

        write_mcp_config(HarnessKind::ClaudeCode, &path, &servers).unwrap();

        let result = read_mcp_config(HarnessKind::ClaudeCode, &path).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("existing"));
        assert!(result.contains_key("new-server"));
    }

    #[test]
    fn write_preserves_other_config_fields() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(&path, r#"{"model": "claude-4", "mcpServers": {}}"#).unwrap();

        let mut servers = HashMap::new();
        servers.insert("mcp".to_string(), serde_json::json!({"command": "test"}));

        write_mcp_config(HarnessKind::ClaudeCode, &path, &servers).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("model"));
        assert!(content.contains("claude-4"));
    }

    #[test]
    fn mcp_exists_returns_true_for_existing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(
            &path,
            r#"{"mcpServers": {"test-mcp": {"command": "test"}}}"#,
        )
        .unwrap();

        assert!(mcp_exists(HarnessKind::ClaudeCode, &path, "test-mcp").unwrap());
        assert!(!mcp_exists(HarnessKind::ClaudeCode, &path, "nonexistent").unwrap());
    }

    #[test]
    fn mcp_exists_returns_false_for_missing_file() {
        let result = mcp_exists(
            HarnessKind::ClaudeCode,
            Path::new("/nonexistent/path.json"),
            "any",
        );
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn goose_yaml_preserves_comments() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.yaml");
        fs::write(
            &path,
            r#"# Main configuration
GOOSE_PROVIDER: anthropic
GOOSE_MODEL: claude-sonnet-4  # Best model

# Extensions section
extensions:
  developer:
    enabled: true
    type: builtin
"#,
        )
        .unwrap();

        let mut servers = HashMap::new();
        servers.insert(
            "new-mcp".to_string(),
            serde_json::json!({"type": "stdio", "cmd": "npx", "args": ["-y", "server"]}),
        );

        write_mcp_config(HarnessKind::Goose, &path, &servers).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("# Main configuration"),
            "Header comment preserved"
        );
        assert!(content.contains("# Best model"), "Inline comment preserved");
        assert!(
            content.contains("# Extensions section"),
            "Section comment preserved"
        );
        assert!(content.contains("new-mcp"), "New MCP added");
        assert!(
            content.contains("developer"),
            "Existing extension preserved"
        );
    }

    #[test]
    fn goose_yaml_creates_extensions_if_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.yaml");
        fs::write(
            &path,
            r#"# Config without extensions
GOOSE_PROVIDER: anthropic
"#,
        )
        .unwrap();

        let mut servers = HashMap::new();
        servers.insert(
            "new-mcp".to_string(),
            serde_json::json!({"type": "stdio", "cmd": "test"}),
        );

        write_mcp_config(HarnessKind::Goose, &path, &servers).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("# Config without extensions"),
            "Comment preserved"
        );
        assert!(
            content.contains("extensions:"),
            "Extensions section created"
        );
        assert!(content.contains("new-mcp"), "New MCP added");
    }

    #[test]
    fn goose_yaml_preserves_existing_mcps() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.yaml");
        fs::write(
            &path,
            r#"extensions:
  existing-mcp:
    type: stdio
    cmd: old-command
"#,
        )
        .unwrap();

        let mut servers = HashMap::new();
        servers.insert(
            "new-mcp".to_string(),
            serde_json::json!({"type": "stdio", "cmd": "new-command"}),
        );

        write_mcp_config(HarnessKind::Goose, &path, &servers).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("existing-mcp"), "Existing MCP preserved");
        assert!(
            content.contains("old-command"),
            "Existing MCP config preserved"
        );
        assert!(content.contains("new-mcp"), "New MCP added");
    }
}
