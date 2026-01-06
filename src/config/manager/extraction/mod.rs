use std::path::{Path, PathBuf};

pub use harness_locate::DirectoryStructure;
use harness_locate::{Harness, Scope};

use crate::config::jsonc::strip_jsonc_comments;
use crate::config::types::{McpServerInfo, ResourceSummary};
use crate::error::{Error, Result};
use crate::harness::HarnessConfig;

pub fn extract_mcp_from_opencode_config(profile_path: &Path) -> Result<Vec<McpServerInfo>> {
    let config_path = profile_path.join("opencode.jsonc");
    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| Error::Config(format!("Failed to read opencode.jsonc: {}", e)))?;
    let content = strip_jsonc_comments(&content);

    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| Error::Config(format!("Failed to parse opencode.jsonc: {}", e)))?;

    let mcp_obj = match config.get("mcp").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return Ok(Vec::new()),
    };

    let servers = mcp_obj
        .iter()
        .map(|(name, value)| {
            let server_type = value.get("type").and_then(|v| v.as_str()).map(String::from);
            let command = value
                .get("command")
                .and_then(|v| v.as_str())
                .map(String::from);
            let args = value.get("args").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|a| a.as_str().map(String::from))
                    .collect()
            });
            let url = value.get("url").and_then(|v| v.as_str()).map(String::from);
            McpServerInfo {
                name: name.clone(),
                enabled: true,
                server_type,
                command,
                args,
                url,
            }
        })
        .collect();

    Ok(servers)
}

pub fn extract_mcp_servers(
    harness: &dyn HarnessConfig,
    profile_path: &Path,
) -> Result<Vec<McpServerInfo>> {
    match harness.id() {
        "opencode" => extract_mcp_from_opencode_config(profile_path),
        "amp-code" => extract_mcp_from_ampcode_config(profile_path),
        "claude-code" => extract_mcp_from_claudecode_config(profile_path),
        "goose" => extract_mcp_from_goose_config(profile_path),
        _ => extract_mcp_generic(harness, profile_path),
    }
}

fn extract_mcp_generic(
    harness: &dyn HarnessConfig,
    profile_path: &Path,
) -> Result<Vec<McpServerInfo>> {
    let mcp_filename = match harness.mcp_filename() {
        Some(f) => f,
        None => return Ok(Vec::new()),
    };

    let profile_mcp_path = profile_path.join(&mcp_filename);
    if !profile_mcp_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&profile_mcp_path)?;
    let servers = harness.parse_mcp_servers(&content, &mcp_filename)?;
    Ok(servers
        .into_iter()
        .map(|(name, enabled)| McpServerInfo {
            name,
            enabled,
            server_type: None,
            command: None,
            args: None,
            url: None,
        })
        .collect())
}

fn extract_mcp_from_claudecode_config(profile_path: &Path) -> Result<Vec<McpServerInfo>> {
    let config_path = profile_path.join(".mcp.json");
    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| Error::Config(format!("Failed to read .mcp.json: {}", e)))?;

    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| Error::Config(format!("Failed to parse .mcp.json: {}", e)))?;

    let mcp_obj = match config.get("mcpServers").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return Ok(Vec::new()),
    };

    let servers = mcp_obj
        .iter()
        .map(|(name, value)| {
            let disabled = value
                .get("disabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let command = value
                .get("command")
                .and_then(|v| v.as_str())
                .map(String::from);
            let args = value.get("args").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|a| a.as_str().map(String::from))
                    .collect()
            });
            let url = value.get("url").and_then(|v| v.as_str()).map(String::from);
            McpServerInfo {
                name: name.clone(),
                enabled: !disabled,
                server_type: Some("stdio".to_string()),
                command,
                args,
                url,
            }
        })
        .collect();

    Ok(servers)
}

fn extract_mcp_from_goose_config(profile_path: &Path) -> Result<Vec<McpServerInfo>> {
    let config_path = profile_path.join("config.yaml");
    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| Error::Config(format!("Failed to read config.yaml: {}", e)))?;

    let config: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|e| Error::Config(format!("Failed to parse config.yaml: {}", e)))?;

    let extensions = match config.get("extensions").and_then(|v| v.as_mapping()) {
        Some(obj) => obj,
        None => return Ok(Vec::new()),
    };

    let mcp_types = ["stdio", "sse", "http", "streamable_http"];
    let servers = extensions
        .iter()
        .filter_map(|(name, value)| {
            let name_str = name.as_str()?;
            let ext_type = value.get("type").and_then(|v| v.as_str())?;
            if !mcp_types.contains(&ext_type) {
                return None;
            }
            let enabled = value
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let command = value.get("cmd").and_then(|v| v.as_str()).map(String::from);
            let args = value.get("args").and_then(|v| v.as_sequence()).map(|arr| {
                arr.iter()
                    .filter_map(|a| a.as_str().map(String::from))
                    .collect()
            });
            let url = value.get("url").and_then(|v| v.as_str()).map(String::from);
            Some(McpServerInfo {
                name: name_str.to_string(),
                enabled,
                server_type: Some(ext_type.to_string()),
                command,
                args,
                url,
            })
        })
        .collect();

    Ok(servers)
}

fn extract_mcp_from_ampcode_config(profile_path: &Path) -> Result<Vec<McpServerInfo>> {
    let config_path = profile_path.join("settings.json");
    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| Error::Config(format!("Failed to read settings.json: {}", e)))?;

    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| Error::Config(format!("Failed to parse settings.json: {}", e)))?;

    let mcp_obj = match config.get("amp.mcpServers").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return Ok(Vec::new()),
    };

    let servers = mcp_obj
        .iter()
        .map(|(name, value)| {
            let command = value
                .get("command")
                .and_then(|v| v.as_str())
                .map(String::from);
            let args = value.get("args").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|a| a.as_str().map(String::from))
                    .collect()
            });
            let url = value.get("url").and_then(|v| v.as_str()).map(String::from);
            McpServerInfo {
                name: name.clone(),
                enabled: true,
                server_type: Some("stdio".to_string()),
                command,
                args,
                url,
            }
        })
        .collect();

    Ok(servers)
}

pub fn extract_theme(harness: &dyn HarnessConfig, profile_path: &Path) -> Option<String> {
    match harness.id() {
        "opencode" => {
            let config_path = profile_path.join("opencode.jsonc");
            if !config_path.exists() {
                return None;
            }
            let content = std::fs::read_to_string(&config_path).ok()?;
            let clean_json = strip_jsonc_comments(&content);
            let parsed: serde_json::Value = serde_json::from_str(&clean_json).ok()?;
            parsed
                .get("theme")
                .and_then(|v| v.as_str())
                .map(String::from)
        }
        "goose" => {
            let config_path = profile_path.join("config.yaml");
            let content = std::fs::read_to_string(&config_path).ok()?;
            let parsed: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
            parsed
                .get("GOOSE_CLI_THEME")
                .and_then(|v| v.as_str())
                .map(String::from)
        }
        "amp-code" => {
            let config_path = profile_path.join("settings.json");
            let content = std::fs::read_to_string(&config_path).ok()?;
            let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
            parsed
                .get("amp.theme")
                .and_then(|v| v.as_str())
                .map(String::from)
        }
        "claude-code" => {
            let config_path = profile_path.join("settings.json");
            let content = std::fs::read_to_string(&config_path).ok()?;
            let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
            parsed
                .get("theme")
                .and_then(|v| v.as_str())
                .map(String::from)
        }
        _ => None,
    }
}

pub fn extract_model(harness: &dyn HarnessConfig, profile_path: &Path) -> Option<String> {
    match harness.id() {
        "opencode" => extract_model_opencode(profile_path),
        "claude-code" => extract_model_claude_code(profile_path),
        "goose" => extract_model_goose(profile_path),
        "amp-code" => extract_model_ampcode(profile_path),
        _ => None,
    }
}

fn extract_model_opencode(profile_path: &Path) -> Option<String> {
    let config_path = profile_path.join("opencode.jsonc");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let clean_json = strip_jsonc_comments(&content);
    let parsed: serde_json::Value = serde_json::from_str(&clean_json).ok()?;

    parsed
        .get("model")
        .and_then(|v| v.as_str())
        .or_else(|| {
            parsed
                .get("agent")
                .and_then(|a| a.get("general"))
                .and_then(|g| g.get("model"))
                .and_then(|v| v.as_str())
        })
        .map(String::from)
}

fn extract_model_claude_code(profile_path: &Path) -> Option<String> {
    let config_path = profile_path.join("settings.json");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    parsed
        .get("model")
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_model_goose(profile_path: &Path) -> Option<String> {
    let config_path = profile_path.join("config.yaml");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    parsed
        .get("GOOSE_MODEL")
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_model_ampcode(profile_path: &Path) -> Option<String> {
    let config_path = profile_path.join("settings.json");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;

    // AMP Code uses dotted keys like "amp.model.default" directly containing the model name
    if let Some(model) = parsed.get("amp.model.default").and_then(|v| v.as_str()) {
        return Some(model.to_string());
    }

    // Fallback: nested amp.model object
    parsed
        .get("amp")
        .and_then(|amp| amp.get("model"))
        .and_then(|m| m.as_str())
        .map(String::from)
}

fn dir_name_from_path(path: &Path) -> &str {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("skills")
}

fn fallback_dir_name(primary: &str) -> Option<&'static str> {
    match primary {
        "skill" => Some("skills"),
        "agent" => Some("agents"),
        "command" => Some("commands"),
        _ => None,
    }
}

pub fn extract_skills(harness: &Harness, profile_path: &Path) -> (ResourceSummary, Option<String>) {
    if harness.id() == "amp-code" {
        return extract_ampcode_skills(profile_path);
    }

    match harness.skills(&Scope::Global) {
        Ok(Some(dir)) => {
            let subdir = dir_name_from_path(&dir.path);
            let summary = extract_resource_summary(profile_path, subdir, &dir.structure);
            if !summary.items.is_empty() {
                return (summary, None);
            }
            let md_summary = extract_resource_summary(
                profile_path,
                subdir,
                &DirectoryStructure::Flat {
                    file_pattern: "*.md".to_string(),
                },
            );
            if !md_summary.items.is_empty() || md_summary.directory_exists {
                return (md_summary, None);
            }
            if let Some(fallback) = fallback_dir_name(subdir) {
                let fallback_summary =
                    extract_resource_summary(profile_path, fallback, &dir.structure);
                if !fallback_summary.items.is_empty() {
                    return (fallback_summary, None);
                }
            }
            (summary, None)
        }
        Ok(None) => (ResourceSummary::default(), None),
        Err(e) => (ResourceSummary::default(), Some(format!("skills: {}", e))),
    }
}

fn extract_ampcode_skills(profile_path: &Path) -> (ResourceSummary, Option<String>) {
    let skills_dir = profile_path.join("skills");
    if !skills_dir.exists() {
        return (ResourceSummary::default(), None);
    }

    let entries = match std::fs::read_dir(&skills_dir) {
        Ok(e) => e,
        Err(e) => {
            return (
                ResourceSummary {
                    items: Vec::new(),
                    directory_exists: true,
                },
                Some(format!("skills: {}", e)),
            );
        }
    };

    let items: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| e.path().join("SKILL.md").exists())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();

    (
        ResourceSummary {
            items,
            directory_exists: true,
        },
        None,
    )
}

pub fn extract_commands(
    harness: &Harness,
    profile_path: &Path,
) -> (ResourceSummary, Option<String>) {
    if harness.id() == "goose" {
        return extract_goose_recipes(profile_path);
    }

    if harness.id() == "amp-code" {
        return extract_ampcode_commands(profile_path);
    }

    let dir_result = match harness.commands(&Scope::Global) {
        Ok(Some(dir)) => {
            let subdir = dir_name_from_path(&dir.path);
            let summary = extract_resource_summary(profile_path, subdir, &dir.structure);
            if !summary.items.is_empty() {
                (summary, None)
            } else if let Some(fallback) = fallback_dir_name(subdir) {
                let fallback_summary =
                    extract_resource_summary(profile_path, fallback, &dir.structure);
                if !fallback_summary.items.is_empty() {
                    (fallback_summary, None)
                } else {
                    (summary, None)
                }
            } else {
                (summary, None)
            }
        }
        Ok(None) => (ResourceSummary::default(), None),
        Err(e) => (ResourceSummary::default(), Some(format!("commands: {}", e))),
    };

    if harness.id() == "opencode" {
        let (config_summary, config_err) = extract_commands_from_opencode_config(profile_path);
        let mut merged_items = dir_result.0.items;
        merged_items.extend(config_summary.items);
        merged_items.sort();
        merged_items.dedup();
        return (
            ResourceSummary {
                items: merged_items,
                directory_exists: dir_result.0.directory_exists || config_summary.directory_exists,
            },
            dir_result.1.or(config_err),
        );
    }

    dir_result
}

fn extract_commands_from_opencode_config(profile_path: &Path) -> (ResourceSummary, Option<String>) {
    let config_path = profile_path.join("opencode.jsonc");
    if !config_path.exists() {
        return (ResourceSummary::default(), None);
    }

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => return (ResourceSummary::default(), Some(format!("commands: {}", e))),
    };

    let clean_json = strip_jsonc_comments(&content);
    let parsed: serde_json::Value = match serde_json::from_str(&clean_json) {
        Ok(v) => v,
        Err(e) => return (ResourceSummary::default(), Some(format!("commands: {}", e))),
    };

    let commands = parsed
        .get("command")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    (
        ResourceSummary {
            items: commands,
            directory_exists: false,
        },
        None,
    )
}

fn extract_goose_recipes(profile_path: &Path) -> (ResourceSummary, Option<String>) {
    let commands_dir = profile_path.join("commands");
    let recipes_dir = profile_path.join("recipes");
    let target_dir = if commands_dir.exists() {
        commands_dir
    } else if recipes_dir.exists() {
        recipes_dir
    } else {
        return (ResourceSummary::default(), None);
    };

    let entries = match std::fs::read_dir(&target_dir) {
        Ok(e) => e,
        Err(e) => {
            return (
                ResourceSummary {
                    items: Vec::new(),
                    directory_exists: true,
                },
                Some(format!("recipes: {}", e)),
            );
        }
    };

    let items: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("yaml") | Some("yml") | Some("json") | Some("md")
                )
        })
        .filter_map(|e| {
            e.path()
                .file_stem()
                .and_then(|n| n.to_str())
                .map(String::from)
        })
        .collect();

    (
        ResourceSummary {
            items,
            directory_exists: true,
        },
        None,
    )
}

fn extract_ampcode_commands(profile_path: &Path) -> (ResourceSummary, Option<String>) {
    let commands_dir = profile_path.join("commands");
    if !commands_dir.exists() {
        return (ResourceSummary::default(), None);
    }

    let entries = match std::fs::read_dir(&commands_dir) {
        Ok(e) => e,
        Err(e) => {
            return (
                ResourceSummary {
                    items: Vec::new(),
                    directory_exists: true,
                },
                Some(format!("commands: {}", e)),
            );
        }
    };

    let items: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            e.path()
                .file_stem()
                .and_then(|n| n.to_str())
                .map(String::from)
        })
        .collect();

    (
        ResourceSummary {
            items,
            directory_exists: true,
        },
        None,
    )
}

pub fn extract_plugins(
    harness: &Harness,
    profile_path: &Path,
) -> (Option<ResourceSummary>, Option<String>) {
    if harness.id() == "opencode" {
        return extract_plugins_from_opencode_config(profile_path);
    }

    if harness.id() == "claude-code" {
        return extract_claude_code_plugins(profile_path);
    }

    match harness.plugins(&Scope::Global) {
        Ok(Some(dir)) => (
            Some(extract_resource_summary(
                profile_path,
                "plugins",
                &dir.structure,
            )),
            None,
        ),
        Ok(None) => (None, None),
        Err(e) => (None, Some(format!("plugins: {}", e))),
    }
}

fn extract_plugins_from_opencode_config(
    profile_path: &Path,
) -> (Option<ResourceSummary>, Option<String>) {
    let config_path = profile_path.join("opencode.jsonc");
    if !config_path.exists() {
        return (None, None);
    }

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => return (None, Some(format!("plugins: {}", e))),
    };

    let clean_json = strip_jsonc_comments(&content);
    let parsed: serde_json::Value = match serde_json::from_str(&clean_json) {
        Ok(v) => v,
        Err(e) => return (None, Some(format!("plugins: {}", e))),
    };

    let plugins = parsed
        .get("plugin")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if plugins.is_empty() {
        (None, None)
    } else {
        (
            Some(ResourceSummary {
                items: plugins,
                directory_exists: true,
            }),
            None,
        )
    }
}

fn extract_claude_code_plugins(profile_path: &Path) -> (Option<ResourceSummary>, Option<String>) {
    let marketplace_path = profile_path.join(".claude-plugin").join("marketplace.json");
    if marketplace_path.exists()
        && let Some(result) = parse_marketplace_json(&marketplace_path)
    {
        return result;
    }

    let plugins_dir = profile_path.join("plugins");
    if !plugins_dir.exists() {
        return (None, None);
    }

    let entries = match std::fs::read_dir(&plugins_dir) {
        Ok(e) => e,
        Err(e) => {
            return (
                Some(ResourceSummary {
                    items: Vec::new(),
                    directory_exists: true,
                }),
                Some(format!("plugins: {}", e)),
            );
        }
    };

    let items: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| e.path().join(".claude-plugin").join("plugin.json").exists())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();

    if items.is_empty() {
        (None, None)
    } else {
        (
            Some(ResourceSummary {
                items,
                directory_exists: true,
            }),
            None,
        )
    }
}

fn parse_marketplace_json(path: &Path) -> Option<(Option<ResourceSummary>, Option<String>)> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return Some((None, Some(format!("plugins: {}", e)))),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => return Some((None, Some(format!("plugins: {}", e)))),
    };

    let plugins = parsed
        .get("plugins")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })?;

    if plugins.is_empty() {
        None
    } else {
        Some((
            Some(ResourceSummary {
                items: plugins,
                directory_exists: true,
            }),
            None,
        ))
    }
}

pub fn extract_agents(
    harness: &Harness,
    profile_path: &Path,
) -> (Option<ResourceSummary>, Option<String>) {
    let dir_result = match harness.agents(&Scope::Global) {
        Ok(Some(dir)) => {
            let subdir = dir_name_from_path(&dir.path);
            let summary = extract_resource_summary(profile_path, subdir, &dir.structure);
            if !summary.items.is_empty() {
                (Some(summary), None)
            } else {
                let md_summary = extract_resource_summary(
                    profile_path,
                    subdir,
                    &DirectoryStructure::Flat {
                        file_pattern: "*.md".to_string(),
                    },
                );
                if !md_summary.items.is_empty() || md_summary.directory_exists {
                    (Some(md_summary), None)
                } else if let Some(fallback) = fallback_dir_name(subdir) {
                    let fallback_summary =
                        extract_resource_summary(profile_path, fallback, &dir.structure);
                    if !fallback_summary.items.is_empty() {
                        (Some(fallback_summary), None)
                    } else {
                        (Some(summary), None)
                    }
                } else {
                    (Some(summary), None)
                }
            }
        }
        Ok(None) => extract_agents_fallback(profile_path),
        Err(e) => (None, Some(format!("agents: {}", e))),
    };

    if harness.id() == "opencode" {
        let (config_summary, config_err) = extract_agents_from_opencode_config(profile_path);
        if !config_summary.items.is_empty() {
            let mut merged_items = dir_result
                .0
                .as_ref()
                .map(|s| s.items.clone())
                .unwrap_or_default();
            merged_items.extend(config_summary.items);
            merged_items.sort();
            merged_items.dedup();
            return (
                Some(ResourceSummary {
                    items: merged_items,
                    directory_exists: dir_result
                        .0
                        .as_ref()
                        .map(|s| s.directory_exists)
                        .unwrap_or(false),
                }),
                dir_result.1.or(config_err),
            );
        }
    }

    dir_result
}

fn extract_agents_from_opencode_config(profile_path: &Path) -> (ResourceSummary, Option<String>) {
    let config_path = profile_path.join("opencode.jsonc");
    if !config_path.exists() {
        return (ResourceSummary::default(), None);
    }

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => return (ResourceSummary::default(), Some(format!("agents: {}", e))),
    };

    let clean_json = strip_jsonc_comments(&content);
    let parsed: serde_json::Value = match serde_json::from_str(&clean_json) {
        Ok(v) => v,
        Err(e) => return (ResourceSummary::default(), Some(format!("agents: {}", e))),
    };

    let agents = parsed
        .get("agent")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    (
        ResourceSummary {
            items: agents,
            directory_exists: false,
        },
        None,
    )
}

fn extract_agents_fallback(profile_path: &Path) -> (Option<ResourceSummary>, Option<String>) {
    for subdir in ["agent", "agents"] {
        let dir_path = profile_path.join(subdir);
        if dir_path.exists() && dir_path.is_dir() {
            let summary = extract_resource_summary(
                profile_path,
                subdir,
                &DirectoryStructure::Flat {
                    file_pattern: "*.md".to_string(),
                },
            );
            if !summary.items.is_empty() || summary.directory_exists {
                return (Some(summary), None);
            }
        }
    }
    (None, None)
}

pub fn extract_rules_file(
    harness: &Harness,
    profile_path: &Path,
) -> (Option<PathBuf>, Option<String>) {
    match harness.rules(&Scope::Global) {
        Ok(Some(dir)) => {
            let rules_path = match &dir.structure {
                DirectoryStructure::Flat { file_pattern } => {
                    if file_pattern.contains('*') {
                        find_first_matching_file(profile_path, file_pattern)
                    } else {
                        let path = profile_path.join(file_pattern);
                        if path.exists() { Some(path) } else { None }
                    }
                }
                DirectoryStructure::Nested { file_name, .. } => {
                    let path = profile_path.join(file_name);
                    if path.exists() { Some(path) } else { None }
                }
            };
            (rules_path, None)
        }
        Ok(None) => (None, None),
        Err(e) => (None, Some(format!("rules: {}", e))),
    }
}

fn find_first_matching_file(dir: &Path, pattern: &str) -> Option<PathBuf> {
    let mut matches: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.path())
        .filter(|p| matches_pattern(p.file_name().and_then(|n| n.to_str()), pattern))
        .collect();
    matches.sort();
    matches.into_iter().next()
}

pub fn matches_pattern(filename: Option<&str>, pattern: &str) -> bool {
    let Some(name) = filename else { return false };
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{}", suffix));
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    name == pattern
}

pub fn extract_resource_summary(
    base_path: &Path,
    subdir: &str,
    structure: &DirectoryStructure,
) -> ResourceSummary {
    let dir_path = base_path.join(subdir);

    if !dir_path.exists() {
        return ResourceSummary {
            items: vec![],
            directory_exists: false,
        };
    }

    let items = match structure {
        DirectoryStructure::Flat { file_pattern } => list_files_matching(&dir_path, file_pattern),
        DirectoryStructure::Nested {
            subdir_pattern,
            file_name,
        } => list_subdirs_with_file(&dir_path, subdir_pattern, file_name),
    };

    ResourceSummary {
        items,
        directory_exists: true,
    }
}

pub fn list_files_matching(dir: &Path, pattern: &str) -> Vec<String> {
    std::fs::read_dir(dir)
        .ok()
        .map(|entries| {
            let mut items: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .filter(|e| matches_pattern(e.file_name().to_str(), pattern))
                .filter_map(|e| e.path().file_stem()?.to_str().map(String::from))
                .collect();
            items.sort();
            items
        })
        .unwrap_or_default()
}

pub fn list_subdirs_with_file(dir: &Path, subdir_pattern: &str, file_name: &str) -> Vec<String> {
    std::fs::read_dir(dir)
        .ok()
        .map(|entries| {
            let mut items: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter(|e| matches_pattern(e.file_name().to_str(), subdir_pattern))
                .filter(|e| e.path().join(file_name).exists())
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .collect();
            items.sort();
            items
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn dir_name_from_path_extracts_final_component() {
        assert_eq!(dir_name_from_path(Path::new("/foo/bar/skill")), "skill");
        assert_eq!(dir_name_from_path(Path::new("/foo/bar/skills")), "skills");
        assert_eq!(dir_name_from_path(Path::new("agent")), "agent");
        assert_eq!(dir_name_from_path(Path::new("/command")), "command");
    }

    #[test]
    fn dir_name_from_path_handles_trailing_slash() {
        assert_eq!(dir_name_from_path(Path::new("/foo/bar/skill/")), "skill");
    }

    #[test]
    fn opencode_uses_singular_directory_names() {
        let harness = Harness::new(harness_locate::HarnessKind::OpenCode);

        if let Ok(Some(skills_dir)) = harness.skills(&Scope::Global) {
            let name = dir_name_from_path(&skills_dir.path);
            assert_eq!(
                name, "skill",
                "OpenCode skills directory should be 'skill' (singular)"
            );
        }

        if let Ok(Some(agents_dir)) = harness.agents(&Scope::Global) {
            let name = dir_name_from_path(&agents_dir.path);
            assert_eq!(
                name, "agent",
                "OpenCode agents directory should be 'agent' (singular)"
            );
        }

        if let Ok(Some(commands_dir)) = harness.commands(&Scope::Global) {
            let name = dir_name_from_path(&commands_dir.path);
            assert_eq!(
                name, "command",
                "OpenCode commands directory should be 'command' (singular)"
            );
        }
    }

    #[test]
    fn claude_uses_plural_directory_names() {
        let harness = Harness::new(harness_locate::HarnessKind::ClaudeCode);

        if let Ok(Some(skills_dir)) = harness.skills(&Scope::Global) {
            let name = dir_name_from_path(&skills_dir.path);
            assert_eq!(
                name, "skills",
                "Claude skills directory should be 'skills' (plural)"
            );
        }
    }

    #[test]
    fn fallback_dir_name_maps_singular_to_plural() {
        assert_eq!(fallback_dir_name("skill"), Some("skills"));
        assert_eq!(fallback_dir_name("agent"), Some("agents"));
        assert_eq!(fallback_dir_name("command"), Some("commands"));
        assert_eq!(fallback_dir_name("skills"), None);
        assert_eq!(fallback_dir_name("agents"), None);
        assert_eq!(fallback_dir_name("other"), None);
    }
}
