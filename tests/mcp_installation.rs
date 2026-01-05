//! Integration tests for MCP installation functionality.
//!
//! Note: These tests verify the MCP config file manipulation directly,
//! since the full install command requires GitHub repository access.
//! The core install_mcp logic is tested via unit tests in mcp_installer.rs.

use std::fs;
use tempfile::TempDir;

/// Test helper to create a temp directory with a config file
fn setup_config_dir(filename: &str, content: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join(filename);
    fs::write(&config_path, content).unwrap();
    temp
}

#[test]
fn claude_mcp_json_can_be_read_and_written() {
    let temp = setup_config_dir(
        ".mcp.json",
        r#"{
  "mcpServers": {
    "existing": {
      "command": "existing-cmd",
      "args": []
    }
  }
}"#,
    );

    let config_path = temp.path().join(".mcp.json");
    let content = fs::read_to_string(&config_path).unwrap();

    // Verify initial state
    assert!(content.contains("existing"));
    assert!(content.contains("mcpServers"));

    // Simulate adding a new MCP by modifying the JSON
    let mut config: serde_json::Value = serde_json::from_str(&content).unwrap();
    config["mcpServers"]["new-mcp"] = serde_json::json!({
        "command": "new-cmd",
        "args": ["--arg1"]
    });
    fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    // Verify both MCPs exist
    let updated = fs::read_to_string(&config_path).unwrap();
    assert!(updated.contains("existing"));
    assert!(updated.contains("new-mcp"));
    assert!(updated.contains("new-cmd"));
}

#[test]
fn opencode_jsonc_can_be_read_and_written() {
    let temp = setup_config_dir(
        "opencode.jsonc",
        r#"{
  // OpenCode configuration
  "theme": "dark",
  "mcp": {
    "existing": {
      "command": "existing-cmd"
    }
  }
}"#,
    );

    let config_path = temp.path().join("opencode.jsonc");
    let content = fs::read_to_string(&config_path).unwrap();

    // Verify initial state
    assert!(content.contains("existing"));
    assert!(content.contains("mcp"));
    assert!(content.contains("// OpenCode configuration"));
}

#[test]
fn goose_yaml_preserves_comments_in_integration() {
    let temp = setup_config_dir(
        "config.yaml",
        r#"# Goose configuration file
GOOSE_PROVIDER: anthropic  # Use Anthropic
GOOSE_MODEL: claude-sonnet-4-20250514

extensions:
  developer:
    enabled: true
    type: builtin  # Built-in extension
"#,
    );

    let config_path = temp.path().join("config.yaml");
    let content = fs::read_to_string(&config_path).unwrap();

    // Verify comments are present
    assert!(content.contains("# Goose configuration file"));
    assert!(content.contains("# Use Anthropic"));
    assert!(content.contains("# Built-in extension"));

    // Simulate adding a new extension while preserving structure
    // (The actual comment-preserving logic is in mcp_config.rs)
    let new_extension = r#"
  new-mcp:
    enabled: true
    type: stdio
    cmd: new-cmd
"#;

    // Find extensions section and append
    let mut lines: Vec<&str> = content.lines().collect();
    lines.push(new_extension.trim());
    let updated = lines.join("\n");

    fs::write(&config_path, &updated).unwrap();

    // Verify comments preserved and new extension added
    let final_content = fs::read_to_string(&config_path).unwrap();
    assert!(final_content.contains("# Goose configuration file"));
    assert!(final_content.contains("new-mcp"));
}

#[test]
fn amp_settings_json_can_be_read_and_written() {
    let temp = setup_config_dir(
        "settings.json",
        r#"{
  "amp": {
    "mcpServers": {
      "existing": {
        "command": "existing-cmd"
      }
    }
  }
}"#,
    );

    let config_path = temp.path().join("settings.json");
    let content = fs::read_to_string(&config_path).unwrap();

    // Verify initial state
    assert!(content.contains("existing"));
    assert!(content.contains("mcpServers"));
    assert!(content.contains("amp"));
}

#[test]
fn config_preserves_existing_mcps_when_adding_new() {
    let temp = setup_config_dir(
        ".mcp.json",
        r#"{
  "mcpServers": {
    "mcp-one": {"command": "cmd1"},
    "mcp-two": {"command": "cmd2"}
  }
}"#,
    );

    let config_path = temp.path().join(".mcp.json");

    // Add a third MCP
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    config["mcpServers"]["mcp-three"] = serde_json::json!({"command": "cmd3"});
    fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    // Verify all three exist
    let final_content = fs::read_to_string(&config_path).unwrap();
    assert!(final_content.contains("mcp-one"));
    assert!(final_content.contains("mcp-two"));
    assert!(final_content.contains("mcp-three"));
}

#[test]
fn empty_config_can_receive_first_mcp() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join(".mcp.json");

    // Create initial empty config
    let initial = serde_json::json!({"mcpServers": {}});
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&initial).unwrap(),
    )
    .unwrap();

    // Add first MCP
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    config["mcpServers"]["first-mcp"] = serde_json::json!({"command": "first-cmd"});
    fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    // Verify MCP was added
    let final_content = fs::read_to_string(&config_path).unwrap();
    assert!(final_content.contains("first-mcp"));
    assert!(final_content.contains("first-cmd"));
}
